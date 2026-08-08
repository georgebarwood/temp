use crate::*;

/// Run stack, Dict, PageSet.
pub struct Run<'a> {
    /// Stack of values that store local variables, function parameters and function result.
    pub stack: LVec<Value>,
    pub dict: &'a Dict,
    pub ps: &'a mut PageSet,
    pub source: LRc<LString>, // For string constants when executing batch.
    pub dict_changed: bool,
    pub error: bool,
    pub new_dict: &'a mut Arc<Dict>,
    pub tr: &'a mut dyn Transaction,
    pub batch: LVec<LRc<LString>>, // Executed after normal execution.
}

impl<'a> Run<'a> {
    /// Create Run.
    pub fn new(
        dict: &'a Dict,
        new_dict: &'a mut Arc<Dict>,
        ps: &'a mut PageSet,
        tr: &'a mut dyn Transaction,
    ) -> Self {
        Self {
            stack: LVec::new(),
            dict,
            ps,
            source: LRc::new(LString::new()),
            new_dict,
            dict_changed: false,
            error: false,
            tr,
            batch: LVec::new(),
        }
    }

    /// Output Value.
    pub fn output(&mut self, v: &Value) {
        match v {
            Value::String(v) => self.tr.output(v.as_bytes()),
            Value::Binary(v) => self.tr.output(v),
            _ => {
                let s = val_to_str(v);
                self.tr.output(s.as_bytes());
            }
        }
    }

    /// Get Function and push default value for result onto stack.
    pub fn call_init(&mut self, f: usize) -> &'a SFunc<NoString> {
        let f = self.dict.func(f);
        let def = f.ret.default_value();
        self.stack.push(def);
        f
    }

    /// Get mut ref to local stack variable.
    pub fn local(&mut self, ix: usize) -> &mut Value {
        let ix = self.stack.len() - (ix + 1);
        &mut self.stack[ix]
    }

    /// Load table specified by table_ix.
    pub fn load_table(&mut self, table_ix: usize) -> RTable {
        let st = self.dict.stable(table_ix);
        self.ps.load_table(st.table_id, &st.dt)
    }

    /// Check transaction is not read_only.
    pub fn check_write(&self) -> Result<(), E> {
        if self.tr.read_only() {
            Err(E::new("Transaction is read only"))
        } else {
            Ok(())
        }
    }
}

/// Executes a batch of statements. Result is None is there was an error, otherwise position in source.
pub fn go(run: &mut Run, nested: bool) -> Option<usize> {
    let source = run.source.clone();

    for pass in 1..=2 {
        let temp_dict = run.new_dict.clone();
        let mut parser = Parser::new(source.as_bytes(), &temp_dict);
        match parser.pass(pass, nested) {
            Err(e) => {
                let pos = parser.position();
                let start = parser.statement_pos;
                let src = tos(&run.source.as_bytes()[start..pos]);
                let dots = if start > 0 { "..." } else { "" };

                let errmsg = format!(
                    "Error {} at input position {}. Source: {}{}",
                    e.message, pos, dots, src
                );
                run.tr.set_error(&errmsg);
                run.error = true;
                println!("{}", errmsg);
                return None;
            }
            Ok(mut slist) => {
                if parser.schema_updates {
                    let md = Arc::make_mut(run.new_dict);
                    if let Err(e) = execute_schema_updates(
                        pass,
                        &slist,
                        source.as_bytes(),
                        md,
                        run.ps,
                        run.tr.read_only(),
                    ) {
                        run.tr.set_error(&e.message);
                        println!("Error {}", e.message);
                        run.error = true;
                        return None;
                    }
                    run.dict_changed = true;
                } else if pass == 2 {
                    encode_block(&mut slist);
                    // println!("Executing {:?}", slist);
                    if let Err(e) = execute_block(&slist, run) {
                        run.tr.set_error(&e.message);
                        println!("Run error {}", e.message);
                        run.error = true;
                        return None;
                    }
                }
                if pass == 2 {
                    return Some(parser.tr.pos);
                }
            }
        }
    }
    None // We do not get here.
}

fn execute_schema_updates(
    pass: u8,
    slist: &[LStatement],
    src: &[u8],
    dict: &mut Dict,
    ps: &mut PageSet,
    read_only: bool,
) -> Result<(), E> {
    if read_only {
        return Err(E::new("Read only transaction cannot update schema"));
    }
    for s in slist {
        if pass == 1 || matches!(s, Statement::CreateFn(_)) {
            match s {
                Statement::Null => {}
                Statement::CreateSchema(x) => {
                    let sname = x.sname.sstr(src);
                    if dict.schema_id(sname).is_some() {
                        return Err(E::new("Duplicate schema name"));
                    }
                    dict.create_schema(sname);
                }
                Statement::RenameSchema(x) => {
                    let new_name = x.new_name.sstr(src);
                    dict.rename_schema(x.schema_id, new_name);
                }
                Statement::CreateTable(x) => {
                    let tname = x.tname.sstr(src);
                    if dict.table(x.schema_id, tname).is_some() {
                        return Err(E::new("Duplicate table name"));
                    }
                    let ix = dict.create_table(x.schema_id, tname, &x.col_defs);
                    let st = dict.stable(ix);
                    let _ = ps.load_table(st.table_id, &st.dt); // Trigger creation of table or reading it will produce an error later.
                }
                Statement::AddColumn(x) => {
                    let tix = x.table_ix;
                    let st = dict.stable(tix);
                    let t = ps.load_table(st.table_id, &st.dt);
                    let recs = t.borrow().record_count();
                    if recs > 0 {
                        return Err(E::new("Add Col, record count > 0"));
                    }
                    let col_name = x.col_name.sstr(src);
                    let dt = dict.add_column(tix, col_name, &x.col_dt);
                    t.borrow_mut().set_datatype(dt);
                }
                Statement::RenameColumn(x) => {
                    let new_name = x.new_name.sstr(src);
                    let tix = x.table_ix;
                    let dt = dict.rename_column(tix, x.col_num, new_name);
                    let st = dict.stable(tix);
                    let t = ps.load_table(st.table_id, &dt);
                    t.borrow_mut().set_datatype(dt);
                }
                Statement::DropColumn(x) => {
                    let tix = x.table_ix;
                    if dict.col_is_referenced(tix, x.col_num) {
                        return Err(E::new("Cannot drop referenced column"));
                    }
                    let dt = dict.drop_column(tix, x.col_num);
                    let st = dict.stable(tix);
                    let t = ps.load_table(st.table_id, &dt);
                    t.borrow_mut().set_datatype(dt);
                }
                Statement::RenameTable(x) => dict.rename_table(x, src),
                Statement::CreateFn(x) => {
                    if pass == 1 && !x.alter {
                        let fname = x.fname.sstr(src);
                        if dict.func_index(x.schema_id, fname).is_some() {
                            return Err(E::new("Duplicate function name"));
                        }
                        dict.create_fn(x, src);
                    } else if pass == 2 {
                        dict.set_fn_block(x, src);
                    }
                }
                Statement::RenameFn(x) => dict.rename_fn(x, src),
                Statement::DropSchema(x) => {
                    if dict.schema_is_referenced(x.schema_id) {
                        return Err(E::new("Cannot drop referenced schema"));
                    }
                    dict.drop_schema(x.schema_id);
                }
                Statement::DropFn(x) => {
                    if dict.fn_is_referenced(x.function_id) {
                        return Err(E::new("Cannot drop referenced function"));
                    }
                    dict.drop_fn(x.function_id);
                }
                Statement::DropTable(x) => {
                    if dict.table_is_referenced(x.table) {
                        return Err(E::new("Cannot drop referenced table"));
                    }
                    let tix = x.table;
                    let st = dict.stable(tix).clone();
                    dict.drop_table(tix);
                    // Remove record from sys_schema.
                    Table::drop(st.table_id, st.dt.clone(), ps);
                }
                Statement::AddIndex(_x) => {
                    println!("Add Index not yet implementd");
                }
                _ => {
                    // println!("s={:?}", s);
                    panic!();
                }
            }
        }
    }
    Ok(())
}

/// Encode a list of statements, optimising where conditions if possible.
pub fn encode_block<A, S>(slist: &mut [Statement<A, S>])
where
    A: Allocator + Debug + Default,
    S: XString,
{
    for s in slist {
        let mut sub_s = None; // Optimised statement to be assigned to s.
        match s {
            Statement::Let(x) => x.exp.encode(),
            Statement::Set(x) => x.exp.encode(),
            Statement::Append(x) => x.exp.encode(),
            Statement::While(x) => {
                x.exp.encode();
                encode_block(&mut x.block);
            }
            Statement::If(x) => {
                x.exp.encode();
                encode_block(&mut x.block);
                if let Some(ref mut els) = x.els {
                    encode_block(els);
                }
            }
            Statement::Insert(x) => encode_exp_list(&mut x.vals),
            Statement::Update(x) => {
                for (_, exp) in &mut x.assigns {
                    exp.encode();
                }
                if let Some(exp) = encode_wher(&mut x.wher) {
                    let assigns = std::mem::take(&mut x.assigns);
                    let table = x.table;
                    sub_s = Some(Statement::UpdateIdEq(UpdateIdEq {
                        assigns,
                        table,
                        exp,
                    }));
                }
            }
            Statement::Delete(x) => {
                if let Some(exp) = encode_wher(&mut x.wher) {
                    let table = x.table;
                    sub_s = Some(Statement::DeleteIdEq(DeleteIdEq { table, exp }));
                }
            }
            Statement::Select(x) => {
                encode_exp_list(&mut x.vals);
                if let Some(ref mut wher) = x.wher
                    && let Some(exp) = encode_wher(wher)
                {
                    let vals = std::mem::take(&mut x.vals);
                    let from = x.from;
                    sub_s = Some(Statement::SelectIdEq(SelectIdEq { vals, from, exp }));
                } else if let Some(ref mut ob) = x.order_by {
                    for exp in &mut ob.0 {
                        exp.encode();
                    }
                }
            }
            Statement::For(x) => {
                for (_, exp) in &mut x.assigns {
                    exp.encode();
                }
                encode_block(&mut x.block);
                if let Some(ref mut wher) = x.wher
                    && let Some(exp) = encode_wher(wher)
                {
                    let assigns = std::mem::take(&mut x.assigns);
                    let from = x.from;
                    let block = std::mem::take(&mut x.block);
                    sub_s = Some(Statement::ForIdEq(ForIdEq {
                        assigns,
                        from,
                        exp,
                        block,
                    }));
                } else if let Some(ref mut ob) = x.order_by {
                    for exp in &mut ob.0 {
                        exp.encode();
                    }
                }
            }
            _ => {}
        }
        if let Some(ss) = sub_s {
            *s = ss;
        }
    }
}

/// Encode where expression, returns Exp for case Id = `<exp>` where exp has no column references.
fn encode_wher<A>(wher: &mut Exp<A>) -> Option<Exp<A>>
where
    A: Allocator + Debug + Default,
{
    // Check conditions for Id = exp optimisation to apply.
    if let Exp::Binary(Operator::Equal,lhs,rhs) = wher
         && let Exp::Col(0) = &**lhs
         && !rhs.has_col()
    {
        rhs.encode();
        Some(std::mem::take(rhs))
    } else {
        wher.encode();
        None
    }
}

fn encode_exp_list<A: Allocator + Debug + Default>(list: &mut [Exp<A>]) {
    for exp in list {
        exp.encode();
    }
}

/// Append to String or Binary Value.
pub fn append(x: &mut Value, y: &Value) {
    // Could use get_mut + with_capacity instead of make_mut.
    match (x, y) {
        (Value::String(x), Value::String(y)) => LRc::make_mut(x).push_str(y),
        (Value::Binary(x), Value::Binary(y)) => LRc::make_mut(x).extend_from_slice(y),
        _ => panic!(),
    }
}

/// Compare table rows.
pub fn row_compare(a: &[Value], b: &[Value], desc: &[bool]) -> std::cmp::Ordering {
    use std::cmp::Ordering;
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
