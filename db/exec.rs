use crate::*;

/// Run stack, Dict, PageSet.
pub struct Run<'a> {
    /// Stack of values that store local variables, function parameters and function result.
    pub stack: LVec<Value>,
    pub dict: &'a Dict,
    pub ps: &'a mut PageSet,
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
                    execute_schema_updates(pass, &slist, md, ps);
                    update_dict = true;
                } else if pass == 2 {
                    let mut run = Run {
                        stack: LVec::new(),
                        dict: parser.dict,
                        ps,
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

fn execute_schema_updates(pass: u8, slist: &[Statement], dict: &mut Dict, ps: &mut PageSet) {
    for s in slist {
        // println!("Pass={} executing {:?}", pass, s);
        match s {
            Statement::CreateSchema(cs) => {
                if pass == 2 {
                    let schema_id = dict.main.new_schema_id();
                    let s = GString::from(cs.sname);
                    dict.main.schemas.insert(s, schema_id);
                    println!("Schema {} created", &cs.sname);
                }
            }

            Statement::CreateTable(x) => {
                if pass == 1 {
                    let id = dict.main.new_table_id();
                    let table = STable {
                        id,
                        dt: x.col_defs.clone(),
                    };
                    println!("Table Created {:?}", &table);
                    let nid = dict.main.new_name_id(x.tname);
                    dict.main.tables.insert((x.schema_id, nid), Arc::new(table));
                }
            }

            Statement::RenameTable(x) => {
                if pass == 1 {
                    let t = dict.main.tables.remove(&(x.old_schema_id, x.old_nid)).unwrap();
                    let new_nid = dict.main.new_name_id(x.new_tname);
                    dict.main.tables.insert((x.new_schema_id, new_nid), t);
                }
            }

            Statement::CreateFn(cf) => {
                if pass == 1 {
                    let func_id = dict.main.funcs.len();
                    let nid = dict.main.new_name_id(cf.fname);
                    let block = GVec::new(); // Dummy block on pass 1
                    let mut parms = GVec::new();
                    for (name, typ) in &cf.parms {
                        parms.push((NoString::from_str(name), typ.clone()));
                    }
                    let func = SFunc::<NoString> {
                        schema_id: cf.schema_id,
                        fname: NoString::from_str(cf.fname),

                        ret: cf.ret.clone(),
                        parms,
                        block,
                    };
                    dict.main.funcs.push(func);
                    dict.main.func_lookup.insert((cf.schema_id, nid), func_id);
                    // ToDo: update info dict as well.
                } else {
                    // Set the function block.
                    let nid = dict.main.names.get(cf.fname).unwrap();
                    let fid = dict.main.func_lookup.get(&(cf.schema_id, *nid)).unwrap();
                    let f = &mut dict.main.funcs[*fid];
                    f.block = gblock(&cf.block);
                }
            }

            Statement::RenameFn(x) => {
                if pass == 1 {
                    let f = dict.main.func_lookup
                        .remove(&(x.old_schema_id, x.old_nid))
                        .unwrap();
                    let new_nid = dict.main.new_name_id(x.new_fname);
                    dict.main.func_lookup.insert((x.new_schema_id, new_nid), f);
                }
            }

            Statement::DropTable(dt) => {
                if pass == 1 {
                    dict.main.tables.remove(&(dt.schema_id, dt.name_id));
                    // Remove record from sys_schema using dt.table_id and ps.
                    Table::drop(dt.table.id, dt.table.dt.clone(), ps);
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
