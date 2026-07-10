use crate::*;
use std::cell::RefCell;

/// Executes a batch of statements. Result is whether dict was updated.
pub fn go(source: &[u8], dict: &mut Arc<Dict>, ps: &mut PageSet) -> bool {
    let dc = dict.clone();
    let mut parser = Parser::new(source, &dc);

    let mut dict_updated = false;

    println!("go source={}", tos(source));

    // Should be a loop here - parser.statements can return without exhausting input ("GO").
    match parser.statements() {
        Err(e) => {
            let pos = parser.position();
            println!("Error {} at input position {}", e._message, pos);
            println!("Source: {}", tos(&source[0..pos]));
            println!();
        }
        Ok(slist) => {
            if parser.schema_updates {
                // println!("statements={:#?}", &slist);
                let md = Arc::make_mut(dict);
                execute_schema_updates(&slist, md, ps);
                dict_updated = true;
            } else {
                execute(&slist, source, ps);
            }
        }
    }
    dict_updated
}

fn execute_schema_updates(slist: &[(usize, Statement)], dict: &mut Dict, ps: &mut PageSet) {
    for (_pos, s) in slist {
        println!("executing {:?}", s);
        match s {
            Statement::CreateTable(ct) => {
                let id = dict.new_table_id();
                let table = STable {
                    id,
                    dt: ct.col_defs.clone(),
                };
                let nid = dict.new_name_id(ct.tname);
                dict.tables.insert((ct.schema_id, nid), Arc::new(table));
            }

            Statement::DropTable(dt) => {
                dict.tables.remove(&(dt.schema_id, dt.name_id));
                // Remove record from sys_schema using dt.table_id and ps.
                Table::drop(dt.table.id, dt.table.dt.clone(), ps);
            }

            Statement::CreateSchema(cs) => {
                let schema_id = dict.new_schema_id();
                let s = GString::from(cs.sname);
                dict.schemas.insert(s, schema_id);
                println!("Schema {} created", cs.sname);
            }
            _ => todo!(),
        }
    }
}

fn execute(slist: &[(usize, Statement)], source: &[u8], ps: &mut PageSet) {
    for (pos, s) in slist {
        // println!("executing {:?} position={}", s, pos);
        let result = match s {
            Statement::Insert(x) => exec_insert(x, ps),
            Statement::Update(x) => exec_update(x, ps),
            Statement::Delete(x) => exec_delete(x, ps),
            Statement::Select(x) => exec_select(x, ps),
            _ => todo!(),
        };

        if let Err(e) = &result {
            println!("Error {} at {}", e._message, pos);
            println!("Source: {}", tos(&source[0..*pos]));
            println!();
        }
    }
}

fn ids(t: &LRc<RefCell<Table>>, wher: &Exp, ps: &mut PageSet) -> LVec<i64> {
    let mut result = LVec::new();
    {
        let table = t.borrow();
        let mut iter = table.iter(ps);
        while let Some(b) = iter.next_ref(ps) {
            let mut lr = table.lazy_row(b);
            let id = lr.item(0, ps).int();
            let ok = {
                let mut lrc = Context::LazyRow(&mut lr, &Context::None);
                wher.eval(&mut lrc, ps).bool()
            };
            if ok {
                result.push(id);
            }
        }
    }
    result
}

fn exec_insert(ins: &Insert, ps: &mut PageSet) -> Result<(), E> {
    // println!("ins={:?}", ins);

    // First evaluate the expressions.
    let mut ee = LVec::with_capacity(ins.vals.len());
    for e in &ins.vals {
        ee.push(e.eval(&mut Context::None, ps));
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

fn exec_update(upd: &Update, ps: &mut PageSet) -> Result<(), E> {
    // println!("upd={:?}", upd);

    let t = ps.load_table(upd.table.id, &upd.table.dt);

    let ids = ids(&t, &upd.wher, ps);

    // println!("ids to be updated={:?}", ids);
    let mut table = t.borrow_mut();
    for id in &ids {
        let mut row = table.fetch(*id, ps).unwrap();
        let mut vals = LVec::new();
        {
            let mut ctx = Context::Values(row.list());
            for (_col, e) in &upd.assigns {
                let v = e.eval(&mut ctx, ps);
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

fn exec_delete(del: &Delete, ps: &mut PageSet) -> Result<(), E> {
    let t = ps.load_table(del.table.id, &del.table.dt);
    let ids = ids(&t, &del.wher, ps);
    let mut table = t.borrow_mut();
    for id in &ids {
        table.remove(*id, ps);
    }
    Ok(())
}

fn exec_select(sel: &Select, ps: &mut PageSet) -> Result<(), E> {
    // println!("exec_sel sel={:?}", sel);

    if let Some(f) = &sel.from {
        let t = ps.load_table(f.id, &f.dt);
        let table = t.borrow();

        let mut iter = table.iter(ps);
        while let Some(b) = iter.next_ref(ps) {
            // print!("got a row :");
            let mut lr = table.lazy_row(b);
            let mut lrc = Context::LazyRow(&mut lr, &Context::None);

            let ok = if let Some(wher) = &sel.wher {
                wher.eval(&mut lrc, ps).bool()
            } else {
                true
            };

            if ok {
                print!("Selected vals=");
                for e in &sel.vals {
                    let v = e.eval(&mut lrc, ps);
                    print!(" {:?} ", v);
                }
                println!();
            } else {
                // println!(" row skipped due to where being false");
            }
        }
    } else {
        // SELECT with no FROM
        let mut lr = Context::None;
        for e in &sel.vals {
            let v = e.eval(&mut lr, ps);
            print!(" {:?} ", v);
        }
        println!();
    }

    Ok(())
}
