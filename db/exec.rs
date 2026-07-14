use crate::*;

/// Run stack, Dict, PageSet.
pub struct Run<'a> {
    /// Stack of values that store local variables, function parameters and function result.
    pub stack: LVec<Value>,
    pub dict: &'a Dict,
    pub ps: &'a mut PageSet,
}

/// Executes a batch of statements. Result is whether dict was updated.
pub fn go(source: &[u8], dict: &mut Arc<Dict>, ps: &mut PageSet) -> bool {
    let mut temp_dict = dict.clone();
    let mut update_dict = false;

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
                    let mut run = Run { stack: LVec::new(), dict: parser.dict, ps };
                    execute_block(&slist, &mut run);
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

fn execute_block(slist: &[Statement], run: &mut Run) {
    let slen = run.stack.len(); // At end restore stack to this length.
    for s in slist {
        use Statement::*;
        match s {
            Let(x) => exec_let(x, run),
            Set(x) => exec_set(x, run),
            Append(x) => exec_append(x, run),
            While(x) => exec_while(x, run),
            If(x) => exec_if(x, run),
            Insert(x) => exec_insert(x, run),
            Update(x) => exec_update(x, run),
            Delete(x) => exec_delete(x, run),
            Select(x) => exec_select(x, run),
            For(x) => exec_for(x, run),
            CreateSchema(_) | CreateTable(_) | CreateFn(_) | DropTable(_) => panic!(),
        };
    }
    run.stack.truncate(slen); // pop local variables from stack.
}

fn exec_let(x: &Let, run: &mut Run) {
    let v = x.exp.eval(run);
    run.stack.push(v);
}

fn exec_set(x: &Set, run: &mut Run) {
    let v = x.exp.eval(run);
    let ix = run.stack.len() - 1 - x.i;
    run.stack[ix] = v;
}

fn exec_append(x: &Append, run: &mut Run) {
    let v = x.exp.eval(run);
    let ix = run.stack.len() - 1 - x.i;
    append(&mut run.stack[ix], &v);
}

fn exec_while(x: &While, run: &mut Run) {
    while x.exp.eval(run).bool() {
        execute_block(&x.block, run);
    }
}

fn exec_if(x: &If, run: &mut Run) {
    if x.exp.eval(run).bool() {
        execute_block(&x.block, run);
    } else if let Some(els) = &x.els {
        execute_block(els, run);
    }
}

fn exec_insert(ins: &Insert, run: &mut Run) {
    // First evaluate the expressions.
    let mut ee = LVec::with_capacity(ins.vals.len());
    for e in &ins.vals {
        ee.push(e.eval(run));
    }
    let t = &ins.table;
    let t = run.ps.load_table(t.id, &t.dt);
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
        table.remove(row_id, run.ps); // Remove any existing record before inserting.
    }

    table.insert(&row, run.ps);

    /* println!(
        "ins table record count={} row={:?}",
        table.record_count(),
        row
    ); */
}

fn exec_update(upd: &Update, run: &mut Run) {
    let t = run.ps.load_table(upd.table.id, &upd.table.dt);

    let ids = ids(&t, &upd.wher, run);

    let mut table = t.borrow_mut();
    for id in &ids {
        let mut row = table.fetch(*id, run.ps).unwrap();
        let mut vals = LVec::with_capacity(upd.assigns.len());
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
        table.update(*id, &row, run.ps);
    }
}

fn exec_delete(del: &Delete, run: &mut Run) {
    let t = run.ps.load_table(del.table.id, &del.table.dt);
    let ids = ids(&t, &del.wher, run);
    let mut table = t.borrow_mut();
    for id in &ids {
        table.remove(*id, run.ps);
    }
}

fn exec_select(x: &Select, run: &mut Run) {
    if x.order_by.is_some() {
        exec_select_order_by(x, run)
    } else if let Some(f) = &x.from {
        let t = run.ps.load_table(f.id, &f.dt);
        let table = t.borrow();

        let mut iter = table.iter(run.ps);
        while let Some(b) = iter.next_ref(run.ps) {
            // print!("got a row :");
            let mut lr = table.lazy_row(b);
            let ok = if let Some(wher) = &x.wher {
                wher.eval_lr(run, &mut lr).bool()
            } else {
                true
            };

            if ok {
                print!("Selected vals=");
                for e in &x.vals {
                    let v = e.eval_lr(run, &mut lr);
                    print!(" {:?} ", v);
                }
                println!();
            }
        }
    } else {
        // select with no from
        for e in &x.vals {
            let v = e.eval(run);
            print!(" {:?} ", v);
        }
        println!();
    }
}

fn exec_select_order_by(x: &Select, run: &mut Run) {
    let f = x.from.as_ref().unwrap();
    let temp = get_temp(f, &x.vals, &x.wher, &x.order_by, run);

    let n = x.order_by.as_ref().unwrap().0.len();
    for row in &temp {
        println!("sorted row={:?}", &row[n..]);
    }
}

fn exec_for(x: &For, run: &mut Run) {
    if x.order_by.is_some() {
        exec_for_order_by(x, run);
    } else {
        let t = run.ps.load_table(x.from.id, &x.from.dt);
        let table = t.borrow();
        let mut iter = table.iter(run.ps);
        while let Some(b) = iter.next_ref(run.ps) {
            let mut lr = table.lazy_row(b);

            let ok = if let Some(wher) = &x.wher {
                let v = wher.eval_lr(run, &mut lr);
                v.bool()
            } else {
                true
            };

            if ok {
                let len = run.stack.len();
                for e in &x.vals {
                    let v = e.eval_lr(run, &mut lr);
                    run.stack.push(v);
                }
                execute_block(&x.block, run);
                run.stack.truncate(len);
            }
        }
    }
}

fn exec_for_order_by(x: &For, run: &mut Run) {
    let temp = get_temp(&x.from, &x.vals, &x.wher, &x.order_by, run);

    let n = x.order_by.as_ref().unwrap().0.len();

    for row in &temp {
        let len = run.stack.len();
        for v in &row[n..] {
            run.stack.push(v.clone());
        }
        execute_block(&x.block, run);
        run.stack.truncate(len);
    }
}

/// Get a list of ids for records from table that satisfy where condition.
fn ids(t: &RTable, wher: &Exp, run: &mut Run) -> LVec<i64> {
    let mut result = LVec::new();
    let table = t.borrow();
    let mut iter = table.iter(run.ps);
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        if wher.eval_lr(run, &mut lr).bool() {
            let id = lr.item(0, run.ps).int();
            result.push(id);
        }
    }
    result
}

// Note: statements are GStatement rather than Statement, so need their own execution functions.
pub fn execute_fn(f: &SFunc, run: &mut Run) {
    // println!("execute_fn f={:?}", f);
    execute_gblock(&f.block, run);
}

fn execute_gblock(slist: &[GStatement], run: &mut Run) {
    let slen = run.stack.len(); // At end restore stack to this length.
    for s in slist {
        use GStatement::*;
        match s {
            Let(x) => exec_glet(x, run),
            Set(x) => exec_gset(x, run),
            Append(x) => exec_gappend(x, run),
            While(x) => exec_gwhile(x, run),
            If(x) => exec_gif(x, run),
            Insert(x) => exec_ginsert(x, run),
            Update(x) => exec_gupdate(x, run),
            Delete(x) => exec_gdelete(x, run),
            Select(x) => exec_gselect(x, run),
            For(x) => exec_gfor(x, run),
        };
    }
    run.stack.truncate(slen); // pop local variables from stack.
}

fn exec_glet(x: &GLet, run: &mut Run) {
    let v = x.exp.eval(run);
    run.stack.push(v);
}

fn exec_gset(x: &GSet, run: &mut Run) {
    let v = x.exp.eval(run);
    let ix = run.stack.len() - 1 - x.i;
    run.stack[ix] = v;
}

fn exec_gappend(x: &GAppend, run: &mut Run) {
    let v = x.exp.eval(run);
    let ix = run.stack.len() - 1 - x.i;
    append(&mut run.stack[ix], &v);
}

fn exec_gwhile(x: &GWhile, run: &mut Run) {
    while x.exp.eval(run).bool() {
        execute_gblock(&x.block, run);
    }
}

fn exec_gif(x: &GIf, run: &mut Run) {
    if x.exp.eval(run).bool() {
        execute_gblock(&x.block, run);
    } else if let Some(els) = &x.els {
        execute_gblock(els, run);
    }
}

fn exec_ginsert(ins: &GInsert, run: &mut Run) {
    // First evaluate the expressions.
    let mut ee = LVec::with_capacity(ins.vals.len());
    for e in &ins.vals {
        ee.push(e.eval(run));
    }
    let t = &ins.table;
    let t = run.ps.load_table(t.id, &t.dt);
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
        table.remove(row_id, run.ps); // Remove any existing record before inserting.
    }

    table.insert(&row, run.ps);

    println!(
        "ins table record count={} row={:?}",
        table.record_count(),
        row
    );
}

fn exec_gupdate(upd: &GUpdate, run: &mut Run) {
    let t = run.ps.load_table(upd.table.id, &upd.table.dt);
    let ids = gids(&t, &upd.wher, run);
    let mut table = t.borrow_mut();
    for id in &ids {
        let mut row = table.fetch(*id, run.ps).unwrap();
        let mut vals = LVec::with_capacity(upd.assigns.len());
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
        table.update(*id, &row, run.ps);
    }
}

fn exec_gdelete(del: &GDelete, run: &mut Run) {
    let t = run.ps.load_table(del.table.id, &del.table.dt);
    let ids = gids(&t, &del.wher, run);
    let mut table = t.borrow_mut();
    for id in &ids {
        table.remove(*id, run.ps);
    }
}

fn exec_gselect(sel: &GSelect, run: &mut Run) {
    if sel.order_by.is_some() {
        exec_gselect_order_by(sel, run)
    } else if let Some(f) = &sel.from {
        let t = run.ps.load_table(f.id, &f.dt);
        let table = t.borrow();
        let mut iter = table.iter(run.ps);
        while let Some(b) = iter.next_ref(run.ps) {
            // print!("got a row :");
            let mut lr = table.lazy_row(b);
            let ok = if let Some(wher) = &sel.wher {
                wher.eval_lr(run, &mut lr).bool()
            } else {
                true
            };
            if ok {
                print!("Selected vals=");
                for e in &sel.vals {
                    let v = e.eval_lr(run, &mut lr);
                    print!(" {:?} ", v);
                }
                println!();
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
}

fn exec_gselect_order_by(x: &GSelect, run: &mut Run) {
    let f = x.from.as_ref().unwrap();
    let temp = get_gtemp(f, &x.vals, &x.wher, &x.order_by, run);

    let n = x.order_by.as_ref().unwrap().0.len();
    for row in &temp {
        println!("sorted row={:?}", &row[n..]);
    }
}

fn exec_gfor(x: &GFor, run: &mut Run) {
    if x.order_by.is_some() {
        exec_gfor_order_by(x, run);
    } else {
        let t = run.ps.load_table(x.from.id, &x.from.dt);
        let table = t.borrow();
        let mut iter = table.iter(run.ps);
        while let Some(b) = iter.next_ref(run.ps) {
            let mut lr = table.lazy_row(b);

            let ok = if let Some(wher) = &x.wher {
                let v = wher.eval_lr(run, &mut lr);
                v.bool()
            } else {
                true
            };

            if ok {
                let len = run.stack.len();
                for e in &x.vals {
                    let v = e.eval_lr(run, &mut lr);
                    run.stack.push(v);
                }
                execute_gblock(&x.block, run);
                run.stack.truncate(len);
            }
        }
    }
}

fn exec_gfor_order_by(x: &GFor, run: &mut Run) {
    let temp = get_gtemp(&x.from, &x.vals, &x.wher, &x.order_by, run);

    let n = x.order_by.as_ref().unwrap().0.len();

    for row in &temp {
        let len = run.stack.len();
        for v in &row[n..] {
            run.stack.push(v.clone());
        }
        execute_gblock(&x.block, run);
        run.stack.truncate(len);
    }
}

/// Get a list of ids for records from table that satisfy where condition.
fn gids(t: &RTable, wher: &GExp, run: &mut Run) -> LVec<i64> {
    let mut result = LVec::new();
    let table = t.borrow();
    let mut iter = table.iter(run.ps);
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        if wher.eval_lr(run, &mut lr).bool() {
            let id = lr.item(0, run.ps).int();
             result.push(id);
        }
    }
    result
}

fn append(x: &mut Value, y: &Value) {
    match (x, y) {
        (Value::String(x), Value::String(y)) => LRc::make_mut(x).push_str(y),
        (Value::Binary(x), Value::Binary(y)) => LRc::make_mut(x).extend_from_slice(y),
        _ => panic!(),
    }
}

fn get_temp(
    f: &STable,
    vals: &[Exp],
    wher: &Option<Exp>,
    order_by: &OrderBy,
    run: &mut Run
) -> LVec<LVec<Value>> {
    let (ob, desc) = order_by.as_ref().unwrap();
    // let f = sel.from.as_ref().unwrap();
    let t = run.ps.load_table(f.id, &f.dt);
    let table = t.borrow();
    let mut iter = table.iter(run.ps);

    let mut temp = LVec::new();
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        let ok = if let Some(wher) = &wher {
            wher.eval_lr(run, &mut lr).bool()
        } else {
            true
        };
        if ok {
            let mut row = LVec::with_capacity(ob.len()+vals.len());
            for e in ob {
                let v = e.eval_lr(run, &mut lr);
                row.push(v);
            }
            for e in vals {
                let v = e.eval_lr(run, &mut lr);
                row.push(v);
            }
            temp.push(row);
        }
    }
    temp.sort_by(|a, b| row_compare(a, b, desc));
    temp
}

fn get_gtemp(
    f: &STable,
    vals: &[GExp],
    wher: &Option<GExp>,
    order_by: &GOrderBy,
    run: &mut Run
) -> LVec<LVec<Value>> {
    let (ob, desc) = order_by.as_ref().unwrap();
    let t = run.ps.load_table(f.id, &f.dt);
    let table = t.borrow();
    let mut iter = table.iter(run.ps);

    let mut temp = LVec::new();
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        let ok = if let Some(wher) = &wher {
            wher.eval_lr(run, &mut lr).bool()
        } else {
            true
        };
        if ok {
            let mut row = LVec::with_capacity(ob.len()+vals.len());
            for e in ob {
                let v = e.eval_lr(run, &mut lr);
                row.push(v);
            }
            for e in vals {
                let v = e.eval_lr(run, &mut lr);
                row.push(v);
            }
            temp.push(row);
        }
    }
    temp.sort_by(|a, b| row_compare(a, b, desc));
    temp
}

use std::cmp::Ordering;
/// Compare table rows.
pub fn row_compare(a: &[Value], b: &[Value], desc: &[bool]) -> Ordering {
    let mut ix = 0;
    loop {
        let cmp = a[ix].cmp(&b[ix]);
        if cmp != Ordering::Equal {
            if !desc[ix] {
                return cmp;
            };
            return if cmp == Ordering::Less {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        ix += 1;
        if ix == desc.len() {
            return Ordering::Equal;
        }
    }
}
