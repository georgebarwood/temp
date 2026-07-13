use crate::*;
use std::cell::RefCell;

pub struct Run {
    pub stack: LVec<Value>,
    pub pos: usize, // Error position.
}

/// Executes a batch of statements. Result is whether dict was updated.
pub fn go(source: &[u8], dict: &mut Arc<Dict>, ps: &mut PageSet) -> bool {
    let dc = dict.clone(); // Cloning an Arc is cheap.
    let mut parser = Parser::new(source, &dc);
    let mut temp_dict = dict.clone();
    let mut update_dict = false;

    println!();
    println!("Go source={}", tos(source));

    for pass in 1..=2
    // If we know there are no CREATE FNs, can skip pass 1.
    {
        match parser.pass(pass) {
            Err(e) => {
                let pos = parser.position();
                println!(
                    "Pass {} Error {} at input position {}",
                    pass, e.message, pos
                );
                println!("Source: {}", tos(&source[0..pos]));
                println!();
                update_dict = false;
                break;
            }
            Ok(slist) => {
                if parser.schema_updates {
                    // println!("statements={:#?}", &slist);
                    let md = Arc::make_mut(&mut temp_dict);
                    execute_schema_updates(pass, &slist, md, ps);
                    update_dict = true;
                } else if pass == 2 {
                    let mut run = Run {
                        stack: LVec::new(),
                        pos: 0,
                    };
                    let result = execute_block(&slist, &mut run, ps);
                    if let Err(e) = &result {
                        println!("Error {} at {}", e.message, run.pos);
                        println!("Source: {}", tos(&source[0..run.pos]));
                        println!();
                    }
                }
            }
        }
    }
    if update_dict {
        *dict = temp_dict;
    }
    update_dict
}

fn execute_schema_updates(
    pass: u8,
    slist: &[(usize, Statement)],
    dict: &mut Dict,
    ps: &mut PageSet,
) {
    for (_pos, s) in slist {
        println!("Pass={} executing {:?}", pass, s);
        match s {
            Statement::CreateTable(ct) => {
                if pass == 1 {
                    let id = dict.new_table_id();
                    let table = STable {
                        id,
                        dt: ct.col_defs.clone(),
                    };
                    let nid = dict.new_name_id(ct.tname);
                    dict.tables.insert((ct.schema_id, nid), Arc::new(table));
                }
            }

            Statement::CreateFn(cf) => {
                if pass == 1 {
                    let func_id = 0; // ToDo
                    let nid = dict.new_name_id(cf.fname);
                    let block = GVec::new(); // Dummy block on pass 1
                    let func = SFunc {
                        id: func_id,
                        dt: cf.rtyp.clone(),
                        block,
                    };
                    dict.funcs.insert((cf.schema_id, nid), Arc::new(func));
                } else {
                    // Set the function block.
                    let nid = dict.names.get(cf.fname).unwrap();
                    let mut func = dict.funcs.remove(&(cf.schema_id, *nid)).unwrap();
                    let mf = Arc::make_mut(&mut func);
                    mf.block = gblock(&cf.block);
                    dict.funcs.insert((cf.schema_id, *nid), func);
                }
            }

            Statement::DropTable(dt) => {
                if pass == 1 {
                    dict.tables.remove(&(dt.schema_id, dt.name_id));
                    // Remove record from sys_schema using dt.table_id and ps.
                    Table::drop(dt.table.id, dt.table.dt.clone(), ps);
                }
            }

            Statement::CreateSchema(cs) => {
                if pass == 1 {
                    let schema_id = dict.new_schema_id();
                    let s = GString::from(cs.sname);
                    dict.schemas.insert(s, schema_id);
                    println!("Schema {} created", cs.sname);
                }
            }
            _ => todo!(),
        }
    }
}

fn execute_block(slist: &[(usize, Statement)], run: &mut Run, ps: &mut PageSet) -> Result<(), E> {
    let slen = run.stack.len(); // At end restore stack to this length.
    let mut result = Ok(());
    for (pos, s) in slist {
        // Need to incorporate pos in any error somehow. Maybe have it in run.
        // println!("executing {:?} position={}", s, pos);
        run.pos = *pos;
        result = match s {
            Statement::Insert(x) => exec_insert(x, run, ps),
            Statement::Update(x) => exec_update(x, run, ps),
            Statement::Delete(x) => exec_delete(x, run, ps),
            Statement::Select(x) => exec_select(x, run, ps),
            Statement::Let(x) => exec_let(x, run),
            Statement::Set(x) => exec_set(x, run),
            Statement::While(x) => exec_while(x, run, ps),
            Statement::If(x) => exec_if(x, run, ps),
            Statement::For(x) => exec_for(x, run, ps),
            _ => todo!(),
        };
        if result.is_err() {
            break;
        }
    }
    run.stack.truncate(slen); // pop local variables from stack.
    result
}

fn exec_let(x: &Let, run: &mut Run) -> Result<(), E> {
    let v = x.exp.eval(run);
    run.stack.push(v);
    Ok(())
}

fn exec_for(x: &For, run: &mut Run, ps: &mut PageSet) -> Result<(), E> {
    // Iterate through table. For each row with valid where condition,
    // push evaluated exps on the stack and execute block.

    let t = ps.load_table(x.from.id, &x.from.dt);
    let table = t.borrow();
    let mut iter = table.iter(ps);
    while let Some(b) = iter.next_ref(ps) {
        let mut lr = table.lazy_row(b);

        let ok = if let Some(wher) = &x.wher {
            let v = wher.eval_lr(run, &mut lr, ps);
            v.bool()
        } else {
            true
        };

        if ok {
            let len = run.stack.len();
            for e in &x.vals {
                let v = e.eval_lr(run, &mut lr, ps);
                run.stack.push(v);
            }
            execute_block(&x.block, run, ps)?;
            run.stack.truncate(len);
        }
    }
    Ok(())
}

fn exec_set(x: &Set, run: &mut Run) -> Result<(), E> {
    let v = x.exp.eval(run);
    let ix = run.stack.len() - 1 - x.i;
    run.stack[ix] = v;
    Ok(())
}

fn exec_if(x: &If, run: &mut Run, ps: &mut PageSet) -> Result<(), E> {
    let ok = {
        let v = x.exp.eval(run);
        v.bool()
    };
    if ok {
        execute_block(&x.block, run, ps)?;
    } else if let Some(els) = &x.els {
        execute_block(els, run, ps)?;
    }
    Ok(())
}

fn exec_while(x: &While, run: &mut Run, ps: &mut PageSet) -> Result<(), E> {
    while x.exp.eval(run).bool() {
        execute_block(&x.block, run, ps)?;
    }
    Ok(())
}

fn exec_insert(ins: &Insert, run: &mut Run, ps: &mut PageSet) -> Result<(), E> {
    // println!("ins={:?}", ins);

    // First evaluate the expressions.
    let mut ee = LVec::with_capacity(ins.vals.len());
    for e in &ins.vals {
        ee.push(e.eval(run));
    }
    // println!("ins ee={:?}", &ee );

    let t = &ins.table;
    let t = ps.load_table(t.id, &t.dt);
    let mut table = t.borrow_mut();

    let mut row = table.datatype.default_value();

    let list = row.list_mut();
    let mrow = LRc::make_mut(list);

    // Assign the columns, with the evaluated expressions.
    for (i, e) in ee.into_iter().enumerate() {
        let col = ins.cols[i];
        mrow[col] = e;
    }

    let auto_id = !ins.cols.contains(&0);
    let row_id = if auto_id {
        let row_id = table.new_id();
        mrow[0] = Value::Int(row_id); // Assign the id to the first element.
        row_id
    } else {
        let row_id = mrow[0].int();
        table.reserve_id(row_id);
        row_id
    };

    // if not auto_id, need to check record doesn't already exist.
    if !auto_id && table.fetch(row_id, ps).is_some() {
        // Need some way to report run-time position. Could record it in ins record.
        return Err(E::new("Attempt to insert duplicate record"));
    }

    table.insert(&row, ps);

    println!(
        "ins table record count={} row={:?}",
        table.record_count(),
        row
    );

    Ok(())
}

fn exec_update(upd: &Update, run: &mut Run, ps: &mut PageSet) -> Result<(), E> {
    // println!("upd={:?}", upd);

    let t = ps.load_table(upd.table.id, &upd.table.dt);

    let ids = ids(&t, &upd.wher, run, ps);

    // println!("ids to be updated={:?}", ids);
    let mut table = t.borrow_mut();
    for id in &ids {
        let mut row = table.fetch(*id, ps).unwrap();
        let mut vals = LVec::new();
        {
            for (_col, e) in &upd.assigns {
                let v = e.eval_vals(run, row.list());
                vals.push(v);
            }
        }
        let mrow = LRc::make_mut(row.list_mut());
        for (col, _e) in upd.assigns.iter().rev() {
            mrow[*col] = vals.pop().unwrap();
        }
        table.update(*id, &row, ps);
    }
    Ok(())
}

fn exec_delete(del: &Delete, run: &mut Run, ps: &mut PageSet) -> Result<(), E> {
    let t = ps.load_table(del.table.id, &del.table.dt);
    let ids = ids(&t, &del.wher, run, ps);
    let mut table = t.borrow_mut();
    for id in &ids {
        table.remove(*id, ps);
    }
    Ok(())
}

fn exec_select(sel: &Select, run: &mut Run, ps: &mut PageSet) -> Result<(), E> {
    // println!("exec_sel sel={:?}", sel);

    if let Some(f) = &sel.from {
        let t = ps.load_table(f.id, &f.dt);
        let table = t.borrow();

        let mut iter = table.iter(ps);
        while let Some(b) = iter.next_ref(ps) {
            // print!("got a row :");
            let mut lr = table.lazy_row(b);
            let ok = if let Some(wher) = &sel.wher {
                wher.eval_lr(run, &mut lr, ps).bool()
            } else {
                true
            };

            if ok {
                print!("Selected vals=");
                for e in &sel.vals {
                    let v = e.eval_lr(run, &mut lr, ps);
                    print!(" {:?} ", v);
                }
                println!();
            } else {
                // println!(" row skipped due to where being false");
            }
        }
    } else {
        // SELECT with no FROM
        for e in &sel.vals {
            let v = e.eval(run);
            print!(" {:?} ", v);
        }
        println!();
    }

    Ok(())
}

/// Get a list of ids for records from table that satisfy where condition.
fn ids(t: &LRc<RefCell<Table>>, wher: &Exp, run: &mut Run, ps: &mut PageSet) -> LVec<i64> {
    let mut result = LVec::new();
    {
        let table = t.borrow();
        let mut iter = table.iter(ps);
        while let Some(b) = iter.next_ref(ps) {
            let mut lr = table.lazy_row(b);
            if wher.eval_lr(run, &mut lr, ps).bool() {
                let id = lr.item(0, ps).int();
                result.push(id);
            }
        }
    }
    result
}

// Note: statements are GStatement rather than Statement, so need their own execution functions.
pub fn execute_fn(f: &SFunc, run: &mut Run) -> Result<(), E> {
    println!("execute_fn f={:?}", f);
    for (_pos, s) in &f.block {
        match s {
            GStatement::Set(x) => exec_gset(x, run)?,
            _ => todo!(),
        }
    }
    Ok(())
}

fn exec_gset(x: &GSet, run: &mut Run) -> Result<(), E> {
    let v = x.exp.eval(run);
    let ix = run.stack.len() - 1 - x.i;
    run.stack[ix] = v;
    Ok(())
}
