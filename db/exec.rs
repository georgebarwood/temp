use crate::*;

/// Run stack, Dict, PageSet.
pub struct Run<'a> {
    /// Stack of values that store local variables, function parameters and function result.
    pub stack: LVec<Value>,
    pub dict: &'a Dict,
    pub ps: &'a mut PageSet,
    pub source: &'a [u8],         // For string constants when executing batch.
    pub output: &'a mut LVec<u8>, // Maybe could generalise this in future.
}

impl<'a> Run<'a> {
    /// Output Value.
    pub fn output(&mut self, v: &Value) {
        match v {
            Value::String(v) => self.output.extend_from_slice(v.as_bytes()),
            Value::Binary(v) => self.output.extend_from_slice(v),
            _ => {
                let s = val_to_str(v);
                self.output.extend_from_slice(s.as_bytes());
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
}

/// Executes a batch of statements. Result is whether dict was updated.
pub fn go(source: &[u8], dict: &mut Arc<Dict>, ps: &mut PageSet, output: &mut LVec<u8>) -> bool {
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
            Ok(mut slist) => {
                if parser.schema_updates {
                    // println!("statements={:#?}", &slist);
                    let md = Arc::make_mut(&mut temp_dict);
                    execute_schema_updates(pass, &slist, source, md, ps);
                    update_dict = true;
                } else if pass == 2 {
                    encode_block(&mut slist);
                    let mut run = Run {
                        stack: LVec::new(),
                        dict: parser.dict,
                        ps,
                        source,
                        output,
                    };
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

fn execute_schema_updates(
    pass: u8,
    slist: &[LStatement],
    src: &[u8],
    dict: &mut Dict,
    ps: &mut PageSet,
) {
    for s in slist {
        // println!("Pass={} executing {:?}", pass, s);
        if pass == 1 || matches!(s, Statement::CreateFn(_)) {
            match s {
                Statement::CreateSchema(x) => {
                    let sname = x.sname.sstr(src);
                    dict.create_schema(sname);
                    println!("Schema '{}' created", sname);
                }

                Statement::CreateTable(x) => {
                    let tname = x.tname.sstr(src);
                    dict.create_table(x.schema_id, tname, x.col_defs.clone());
                    println!("Table '{}' created", tname);
                }

                Statement::RenameTable(x) => dict.rename_table(x, src),

                Statement::CreateFn(x) => {
                    if pass == 1 {
                        dict.create_fn(x, src);
                    } else {
                        dict.set_fn_block(x, src);
                    }
                }

                Statement::RenameFn(x) => dict.rename_fn(x, src),

                Statement::DropTable(x) => {
                    dict.drop_table(x);
                    // Remove record from sys_schema using x.table_id and ps.
                    Table::drop(x.table.id, x.table.dt.clone(), ps);
                }
                _ => panic!(),
            }
        }
    }
}

/// Encode a list of statements.
pub fn encode_block<A, S>(slist: &mut [Statement<A, S>])
where
    A: Allocator + Default,
    S: XString,
{
    for s in slist {
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
                if let Some(ref mut els) = x.els
                {
                    encode_block(els);
                }
            }
            Statement::Insert(x) => encode_exp_list( &mut x.vals ),
            Statement::Update(x) => { 
                x.wher.encode();
                for (_,exp) in &mut x.assigns
                {
                    exp.encode();
                }
            },
            Statement::Delete(x) => x.wher.encode(),
            Statement::Select(x) => { 
                encode_exp_list( &mut x.vals );
                if let Some(ref mut wher) = x.wher { wher.encode(); }
                if let Some(ref mut ob) = x.order_by {
                   for exp in &mut ob.0
                   {
                      exp.encode();
                   }
                }
            }
            Statement::For(x) => {
                for (_,exp) in &mut x.lets
                {
                    exp.encode();
                }
                if let Some(ref mut wher) = x.wher { wher.encode(); }
                if let Some(ref mut ob) = x.order_by {
                   for exp in &mut ob.0
                   {
                      exp.encode();
                   }
                }
                encode_block( &mut x.block );
            },
            _ => {}
        }
    }
}

fn encode_exp_list<A: Allocator+Default>( list: &mut [Exp<A>] )
{
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
