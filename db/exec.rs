use crate::*;

/// Executes a batch of statements. Result is whether dict was updated.
pub fn go(batch: &[u8], dict: &mut Arc<Dict>, ps: &mut PageSet) -> bool {
    let dc = dict.clone();
    let mut parser = Parser::new(batch, &dc);

    let mut dict_updated = false;

    // Should be a loop here - parser.statements can return without exhausting input ("GO").    
    match  parser.statements() {
        Err(e) => {
            let pos = parser.position();
            println!("Error {} at input position {}", e._message, pos);
            println!("Source: {}", tos(&parser.tr.input[0..pos]));
        }
        Ok(slist) => { 
            if parser.schema_updates 
            {
                // println!("statements={:#?}", &slist);
                let md = Arc::make_mut(dict);
                execute_schema_updates(&slist, md, ps);
                dict_updated = true;
            } else {
                if let Err(e) = execute(&slist, ps) {
                    println!( "Error {:?}", e );
                    // Shoould return error here.
                };
            }
        }
    }
    dict_updated
}

fn execute_schema_updates(slist: &[(usize,Statement)], dict: &mut Dict, ps: &mut PageSet) {
    for (_pos,s) in slist {
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

fn execute(slist: &[(usize,Statement)], ps: &mut PageSet) -> Result<(),E> {
    for (pos,s) in slist {
        // println!("executing {:?} position={}", s, pos);
        let result = match s {
            Statement::Insert(ins) => {
               exec_insert(ins,ps)
            }
            Statement::Select(sel) => {
               exec_select(sel,ps)
            }
            _ => todo!(),
        };

        if let Err(e) = &result
        {
            println!("Error {:?} at {}", e, pos);
        }
    }
    Ok(())
}

fn exec_insert(ins: &Insert, ps: &mut PageSet) -> Result<(),E> {
    // println!("ins={:?}", ins);

    // First evaluate the expressions.
    let mut ee = LVec::with_capacity(ins.vals.len());
    for e in &ins.vals {
        ee.push( e.eval() );
    }
    // println!("ins ee={:?}", &ee );

    let t = &ins.table;
    let t = ps.load_table(t.id, &t.dt );
    let mut table = t.borrow_mut();

    let mut row = table.datatype.default_value();

    let list = row.list_mut();
    let mrow = LRc::make_mut(list);

    // Assign the columns, with the evaluated expressions.
    for (i, e) in ee.into_iter().enumerate()
    {
        let col = ins.cols[i];
        mrow[col] = e;
    }

    let auto_id = !ins.cols.contains(&0);
    let row_id = if auto_id
    {
        let row_id = table.new_id();
        mrow[0] = Value::Int(row_id); // Assign the id to the first element.
        row_id
    } else {
        let row_id = mrow[0].int();
        table.reserve_id( row_id );
        row_id
    };

    // if not auto_id, need to check record doesn't already exist.
    if !auto_id && table.fetch(row_id, ps).is_some()
    {
       // Need some way to report run-time position. Could record it in ins record.
       return Err( E::new( "Attempt to insert duplicate record" ) );
    }
    
    table.insert(&row, ps);

    println!("ins table record count={} row={:?}", table.record_count(), row );

    Ok(())
}

fn exec_select(sel: &Select, ps: &mut PageSet) -> Result<(),E> {
    println!("todo... sel={:?}", sel);

    let f = &sel.from;
    let t = ps.load_table(f.id, &f.dt );
    let table = t.borrow();

    let mut iter = table.iter(ps);
    while let Some(b) = iter.next_ref(ps)
    {
        println!("got a row");
        let mut lr = table.lazy_row(b);
        for e in &sel.vals
        {
           let v = e.eval_from_row(&mut lr, ps);
           println!("v={:?}", v);
        }
    }
    
    Ok(())
}