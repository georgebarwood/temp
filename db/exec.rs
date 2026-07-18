use crate::*;

/// Run stack, Dict, PageSet.
pub struct Run<'a> {
    /// Stack of values that store local variables, function parameters and function result.
    pub stack: LVec<Value>,
    pub dict: &'a Dict,
    pub ps: &'a mut PageSet,
    pub source: &'a [u8],
    output: &'a mut LVec<u8>, // Maybe could generalise this in future.
}

impl<'a> Run<'a> {
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
            Ok(slist) => {
                if parser.schema_updates {
                    // println!("statements={:#?}", &slist);
                    let md = Arc::make_mut(&mut temp_dict);
                    execute_schema_updates(pass, &slist, source, md, ps);
                    update_dict = true;
                } else if pass == 2 {
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
        match s {
            Statement::CreateSchema(x) => {
                if pass == 1 {
                    let sname = x.sname.str(src);
                    dict.create_schema(sname);
                    println!("Schema '{}' created", sname);
                }
            }

            Statement::CreateTable(x) => {
                if pass == 1 {
                    let tname = x.tname.str(src);
                    dict.create_table( x.schema_id, tname, x.col_defs.clone() );
                    println!("Table '{}' created", tname);
                }
            }

            Statement::RenameTable(x) => {
                if pass == 1 {
                    dict.rename_table( x, src);
                }
            }

            Statement::CreateFn(x) => {
                if pass == 1 {
                    dict.create_fn( x, src );
                } else {
                    dict.set_fn_block( x, src );
                }
            }

            Statement::RenameFn(x) => {
                if pass == 1 {
                    dict.rename_fn( x, src );
                }
            }

            Statement::DropTable(x) => {
                if pass == 1 {
                    dict.drop_table(x);
                    // Remove record from sys_schema using x.table_id and ps.
                    Table::drop(x.table.id, x.table.dt.clone(), ps);
                }
            }
            _ => todo!(),
        }
    }
}

pub fn append(x: &mut Value, y: &Value) {
    // Could use get_mut + with_capacity instead of make_mut.
    match (x, y) {
        (Value::String(x), Value::String(y)) => LRc::make_mut(x).push_str(y),
        (Value::Binary(x), Value::Binary(y)) => LRc::make_mut(x).extend_from_slice(y),
        _ => panic!(),
    }
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
