use crate::*;

/// Stack of values that store local variables, function parameters and function result.
pub struct Run {
    pub stack: LVec<Value>,
}

/// Executes a batch of statements. Result is whether dict was updated.
pub fn go(source: &[u8], dict: &mut Arc<Dict>, ps: &mut PageSet) -> bool {
    let mut temp_dict = dict.clone();
    let mut update_dict = false;

    println!();
    println!("Go source={}", tos(source));

    for pass in 1..=2
    // If we know there are no schema updates, could skip pass 1.
    {
        let parse_dict = temp_dict.clone();
        let mut parser = Parser::new(source, &parse_dict);
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
                    let mut run = Run { stack: LVec::new() };
                    execute_block(&slist, &mut run, parser.dict, ps);
                }
            }
        }
    }
    if update_dict {
        *dict = temp_dict;
        // println!("dict updated to {:?}", &dict);
        // println!();
    }
    update_dict
}

fn execute_schema_updates(pass: u8, slist: &[Statement], dict: &mut Dict, ps: &mut PageSet) {
    for s in slist {
        // println!("Pass={} executing {:?}", pass, s);
        match s {
            Statement::CreateSchema(cs) => {
                if pass == 2 {
                    let schema_id = dict.new_schema_id();
                    let s = GString::from(cs.sname);
                    dict.schemas.insert(s, schema_id);
                    println!("Schema {} created", cs.sname);
                }
            }

            Statement::CreateTable(ct) => {
                if pass == 1 {
                    let id = dict.new_table_id();
                    let table = STable {
                        id,
                        dt: ct.col_defs.clone(),
                    };
                    println!("Table Created {:?}", &table);
                    let nid = dict.new_name_id(ct.tname);
                    dict.tables.insert((ct.schema_id, nid), Arc::new(table));
                }
            }

            Statement::CreateFn(cf) => {
                if pass == 1 {
                    let func_id = dict.funcs.len();
                    let nid = dict.new_name_id(cf.fname);
                    let block = GVec::new(); // Dummy block on pass 1
                    let mut parm_types = GVec::new();
                    for (_name, typ) in &cf.args {
                        parm_types.push(typ.clone());
                    }
                    let func = SFunc {
                        ret: cf.ret.clone(),
                        parm_types,
                        block,
                    };
                    dict.funcs.push(func);
                    dict.func_lookup.insert((cf.schema_id, nid), func_id);
                } else {
                    // Set the function block.
                    let nid = dict.names.get(cf.fname).unwrap();
                    let fid = dict.func_lookup.get(&(cf.schema_id, *nid)).unwrap();
                    let f = &mut dict.funcs[*fid];
                    f.block = gblock(&cf.block);
                }
            }

            Statement::DropTable(dt) => {
                if pass == 1 {
                    dict.tables.remove(&(dt.schema_id, dt.name_id));
                    // Remove record from sys_schema using dt.table_id and ps.
                    Table::drop(dt.table.id, dt.table.dt.clone(), ps);
                }
            }
            _ => todo!(),
        }
    }
}

fn execute_block(slist: &[Statement], run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let slen = run.stack.len(); // At end restore stack to this length.
    for s in slist {
        use Statement::*;
        match s {
            Let(x) => exec_let(x, run, dict, ps),
            Set(x) => exec_set(x, run, dict, ps),
            Append(x) => exec_append(x, run, dict, ps),
            While(x) => exec_while(x, run, dict, ps),
            If(x) => exec_if(x, run, dict, ps),
            Insert(x) => exec_insert(x, run, dict, ps),
            Update(x) => exec_update(x, run, dict, ps),
            Delete(x) => exec_delete(x, run, dict, ps),
            Select(x) => exec_select(x, run, dict, ps),
            For(x) => exec_for(x, run, dict, ps),
            CreateSchema(_) |  CreateTable(_) 
            | CreateFn(_) |  DropTable(_) => panic!()
        };
    }
    run.stack.truncate(slen); // pop local variables from stack.
}

fn exec_let(x: &Let, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let v = x.exp.eval(run, dict, ps);
    run.stack.push(v);
}

fn exec_set(x: &Set, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let v = x.exp.eval(run, dict, ps);
    let ix = run.stack.len() - 1 - x.i;
    run.stack[ix] = v;
}

fn exec_append(x: &Append, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let v = x.exp.eval(run, dict, ps);
    let ix = run.stack.len() - 1 - x.i;
    append( &mut run.stack[ix], &v );
}

fn exec_while(x: &While, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    while x.exp.eval(run, dict, ps).bool() {
        execute_block(&x.block, run, dict, ps);
    }
}

fn exec_if(x: &If, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    if x.exp.eval(run, dict, ps).bool() {
        execute_block(&x.block, run, dict, ps);
    } else if let Some(els) = &x.els {
        execute_block(els, run, dict, ps);
    }
}

fn exec_insert(ins: &Insert, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    // First evaluate the expressions.
    let mut ee = LVec::with_capacity(ins.vals.len());
    for e in &ins.vals {
        ee.push(e.eval(run, dict, ps));
    }
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

    if !auto_id {
        table.remove(row_id, ps); // Remove any existing record before inserting.
    }

    table.insert(&row, ps);

    println!(
        "ins table record count={} row={:?}",
        table.record_count(),
        row
    );
}

fn exec_update(upd: &Update, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let t = ps.load_table(upd.table.id, &upd.table.dt);

    let ids = ids(&t, &upd.wher, run, dict, ps);

    let mut table = t.borrow_mut();
    for id in &ids {
        let mut row = table.fetch(*id, ps).unwrap();
        let mut vals = LVec::new();
        {
            for (_col, e) in &upd.assigns {
                let v = e.eval_vals(run, dict, ps, row.list());
                vals.push(v);
            }
        }
        let mrow = LRc::make_mut(row.list_mut());
        for (col, _e) in upd.assigns.iter().rev() {
            mrow[*col] = vals.pop().unwrap();
        }
        table.update(*id, &row, ps);
    }
}

fn exec_delete(del: &Delete, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let t = ps.load_table(del.table.id, &del.table.dt);
    let ids = ids(&t, &del.wher, run, dict, ps);
    let mut table = t.borrow_mut();
    for id in &ids {
        table.remove(*id, ps);
    }
}

fn exec_select(sel: &Select, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    if let Some(f) = &sel.from {
        let t = ps.load_table(f.id, &f.dt);
        let table = t.borrow();
        let mut iter = table.iter(ps);
        while let Some(b) = iter.next_ref(ps) {
            // print!("got a row :");
            let mut lr = table.lazy_row(b);
            let ok = if let Some(wher) = &sel.wher {
                wher.eval_lr(run, dict, ps, &mut lr).bool()
            } else {
                true
            };

            if ok {
                print!("Selected vals=");
                for e in &sel.vals {
                    let v = e.eval_lr(run, dict, ps, &mut lr);
                    print!(" {:?} ", v);
                }
                println!();
            }
        }
    } else {
        // select with no from
        for e in &sel.vals {
            let v = e.eval(run, dict, ps);
            print!(" {:?} ", v);
        }
        println!();
    }
}

fn exec_for(x: &For, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    // Iterate through table. For each row with valid where condition,
    // push evaluated exps on the stack and execute block.

    let t = ps.load_table(x.from.id, &x.from.dt);
    let table = t.borrow();
    let mut iter = table.iter(ps);
    while let Some(b) = iter.next_ref(ps) {
        let mut lr = table.lazy_row(b);

        let ok = if let Some(wher) = &x.wher {
            let v = wher.eval_lr(run, dict, ps, &mut lr);
            v.bool()
        } else {
            true
        };

        if ok {
            let len = run.stack.len();
            for e in &x.vals {
                let v = e.eval_lr(run, dict, ps, &mut lr);
                run.stack.push(v);
            }
            execute_block(&x.block, run, dict, ps);
            run.stack.truncate(len);
        }
    }
}

/// Get a list of ids for records from table that satisfy where condition.
fn ids(t: &RTable, wher: &Exp, run: &mut Run, dict: &Dict, ps: &mut PageSet) -> LVec<i64> {
    let mut result = LVec::new();
    {
        let table = t.borrow();
        let mut iter = table.iter(ps);
        while let Some(b) = iter.next_ref(ps) {
            let mut lr = table.lazy_row(b);
            if wher.eval_lr(run, dict, ps, &mut lr).bool() {
                let id = lr.item(0, ps).int();
                result.push(id);
            }
        }
    }
    result
}

// Note: statements are GStatement rather than Statement, so need their own execution functions.
pub fn execute_fn(f: &SFunc, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    // println!("execute_fn f={:?}", f);
    execute_gblock(&f.block, run, dict, ps);
}

fn execute_gblock(slist: &[GStatement], run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let slen = run.stack.len(); // At end restore stack to this length.
    for s in slist {
        match s {
            GStatement::Let(x) => exec_glet(x, run, dict, ps),
            GStatement::Set(x) => exec_gset(x, run, dict, ps),
            GStatement::Append(x) => exec_gappend(x, run, dict, ps),
            GStatement::While(x) => exec_gwhile(x, run, dict, ps),
            GStatement::If(x) => exec_gif(x, run, dict, ps),
            GStatement::Insert(x) => exec_ginsert(x, run, dict, ps),
            GStatement::Update(x) => exec_gupdate(x, run, dict, ps),
            GStatement::Delete(x) => exec_gdelete(x, run, dict, ps),
            GStatement::Select(x) => exec_gselect(x, run, dict, ps),
            GStatement::For(x) => exec_gfor(x, run, dict, ps),
        };
    }
    run.stack.truncate(slen); // pop local variables from stack.
}

fn exec_glet(x: &GLet, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let v = x.exp.eval(run, dict, ps);
    run.stack.push(v);
}

fn exec_gset(x: &GSet, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let v = x.exp.eval(run, dict, ps);
    let ix = run.stack.len() - 1 - x.i;
    run.stack[ix] = v;
}

fn exec_gappend(x: &GAppend, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let v = x.exp.eval(run, dict, ps);
    let ix = run.stack.len() - 1 - x.i;
    append( &mut run.stack[ix], &v );
}

fn exec_gwhile(x: &GWhile, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    while x.exp.eval(run, dict, ps).bool() {
        execute_gblock(&x.block, run, dict, ps);
    }
}

fn exec_gif(x: &GIf, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    if x.exp.eval(run, dict, ps).bool() {
        execute_gblock(&x.block, run, dict, ps);
    } else if let Some(els) = &x.els {
        execute_gblock(els, run, dict, ps);
    }
}

fn exec_ginsert(ins: &GInsert, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    // First evaluate the expressions.
    let mut ee = LVec::with_capacity(ins.vals.len());
    for e in &ins.vals {
        ee.push(e.eval(run, dict, ps));
    }
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

    if !auto_id {
        table.remove(row_id, ps); // Remove any existing record before inserting.
    }

    table.insert(&row, ps);

    println!(
        "ins table record count={} row={:?}",
        table.record_count(),
        row
    );
}

fn exec_gupdate(upd: &GUpdate, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let t = ps.load_table(upd.table.id, &upd.table.dt);
    let ids = gids(&t, &upd.wher, run, dict, ps);
    let mut table = t.borrow_mut();
    for id in &ids {
        let mut row = table.fetch(*id, ps).unwrap();
        let mut vals = LVec::new();
        {
            for (_col, e) in &upd.assigns {
                let v = e.eval_vals(run, dict, ps, row.list());
                vals.push(v);
            }
        }
        let mrow = LRc::make_mut(row.list_mut());
        for (col, _e) in upd.assigns.iter().rev() {
            mrow[*col] = vals.pop().unwrap();
        }
        table.update(*id, &row, ps);
    }
}

fn exec_gdelete(del: &GDelete, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    let t = ps.load_table(del.table.id, &del.table.dt);
    let ids = gids(&t, &del.wher, run, dict, ps);
    let mut table = t.borrow_mut();
    for id in &ids {
        table.remove(*id, ps);
    }
}

fn exec_gselect(sel: &GSelect, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    if let Some(f) = &sel.from {
        let t = ps.load_table(f.id, &f.dt);
        let table = t.borrow();
        let mut iter = table.iter(ps);
        while let Some(b) = iter.next_ref(ps) {
            // print!("got a row :");
            let mut lr = table.lazy_row(b);
            let ok = if let Some(wher) = &sel.wher {
                wher.eval_lr(run, dict, ps, &mut lr).bool()
            } else {
                true
            };
            if ok {
                print!("Selected vals=");
                for e in &sel.vals {
                    let v = e.eval_lr(run, dict, ps, &mut lr);
                    print!(" {:?} ", v);
                }
                println!();
           }
        }
    } else {
        // SELECT with no FROM
        for e in &sel.vals {
            let v = e.eval(run, dict, ps);
            print!(" {:?} ", v);
        }
        println!();
    }
}

fn exec_gfor(x: &GFor, run: &mut Run, dict: &Dict, ps: &mut PageSet) {
    // Iterate through table. For each row with valid where condition,
    // push evaluated exps on the stack and execute block.

    let t = ps.load_table(x.from.id, &x.from.dt);
    let table = t.borrow();
    let mut iter = table.iter(ps);
    while let Some(b) = iter.next_ref(ps) {
        let mut lr = table.lazy_row(b);

        let ok = if let Some(wher) = &x.wher {
            let v = wher.eval_lr(run, dict, ps, &mut lr);
            v.bool()
        } else {
            true
        };

        if ok {
            let len = run.stack.len();
            for e in &x.vals {
                let v = e.eval_lr(run, dict, ps, &mut lr);
                run.stack.push(v);
            }
            execute_gblock(&x.block, run, dict, ps);
            run.stack.truncate(len);
        }
    }
}

/// Get a list of ids for records from table that satisfy where condition.
fn gids(t: &RTable, wher: &GExp, run: &mut Run, dict: &Dict, ps: &mut PageSet) -> LVec<i64> {
    let mut result = LVec::new();
    {
        let table = t.borrow();
        let mut iter = table.iter(ps);
        while let Some(b) = iter.next_ref(ps) {
            let mut lr = table.lazy_row(b);
            if wher.eval_lr(run, dict, ps, &mut lr).bool() {
                let id = lr.item(0, ps).int();
                result.push(id);
            }
        }
    }
    result
}

fn append( x: &mut Value, y: &Value )
{
    match (x,y) {
        ( Value::String(x), Value::String(y) ) => {
           let mx = LRc::make_mut(x);
           mx.push_str(y);
        }
        ( Value::Binary(x), Value::Binary(y) ) => {
           let mx = LRc::make_mut(x);
           mx.extend_from_slice(y);
        }
        _ => panic!()
    }
}
