use crate::*;

/// Executes a batch of statements, returning a possibly-updated Dict.
pub fn go(batch: &[u8], dict: &mut Arc<Dict>, ps: &mut PageSet) {
    let dc = dict.clone();
    let mut parser = Parser::new(batch, &dc);

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
                execute_schema_updates(&slist, md);
            } else {
                if let Err(e) = execute(&slist, ps) {
                    println!( "Error {:?} - continuing", e );
                };
            }
        }
    }
}

fn execute_schema_updates(slist: &[Statement], dict: &mut Dict) {
    for s in slist {
        println!("executing {:?}", s);
        match s {
            Statement::CreateTable(ct) => {
                let id = dict.alloc_table_id();
                let table = STable {
                    id,
                    dt: ct.col_defs.clone(),
                };
                let nid = dict.get_name_id(ct.tname);
                dict.tables.insert((ct.schema_id, nid), Arc::new(table));
            }
            Statement::CreateSchema(cs) => {
                let schema_id = dict.alloc_schema_id();
                let s = GString::from(cs.sname);
                dict.schemas.insert(s, schema_id);
                println!("Schema {} created", cs.sname);
            }
            _ => todo!(),
        }
    }
}

fn execute(slist: &[Statement], ps: &mut PageSet) -> Result<(),E> {
    for s in slist {
        println!("executing {:?}", s);
        match s {
            Statement::Insert(ins) => {
               exec_insert(ins,ps)?;
            }
            _ => todo!(),
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

    println!("ins row={:?}", row);

    // if not auto_id, need to check record doesn't already exist.
    if !auto_id && table.fetch(row_id, ps).is_some()
    {
       // Need some way to report run-time position. Could record it in ins record.
       return Err( E::new( "Attempt to insert duplicate record" ) );
    }
    
    table.insert(&row, ps);

    println!("ins table={:?}", table);

    Ok(())
}
