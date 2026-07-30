use crate::*;
use serde::*;

/// Builtin functions
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Builtin {
    len,
    substr,
    replace,
    contains,
    // binlen,
    // binsubstr,
    fn_text,
    table_text,
    table_col_names,
    col_is_referenced,
    table_literal,
    string_literal,
    execute,
    batch,
    arg,
    header,
    parseint,
    error,
    // More to do...
}

impl Builtin {
    pub fn new(name: &[u8]) -> Result<Self, E> {
        use Builtin::*;
        match name {
            b"len" => Ok(len),
            b"substr" => Ok(substr),
            b"replace" => Ok(replace),
            b"contains" => Ok(contains),
            b"fn_text" => Ok(fn_text),
            b"table_text" => Ok(table_text),
            b"table_col_names" => Ok(table_col_names),
            b"col_is_referenced" => Ok(col_is_referenced),
            b"execute" => Ok(execute),
            b"batch" => Ok(batch),
            b"arg" => Ok(arg),
            b"header" => Ok(header),
            b"parseint" => Ok(parseint),
            b"table_literal" => Ok(table_literal),
            b"string_literal" => Ok(string_literal),
            b"error" => Ok(error),
            _ => Err(E::new("Unknown sys call")),
        }
    }

    pub fn eval(&self, run: &mut Run) -> Value {
        // Arguments are on stack
        use Builtin::*;
        match self {
            len => {
                let s = run.stack.pop().unwrap();
                Value::Int(s.string().len() as i64)
            }
            substr => {
                let mut n = run.stack.pop().unwrap().int();
                let mut start = run.stack.pop().unwrap().int();
                let src = run.stack.pop().unwrap();
                let src = src.string();
                if start < 0 {
                    start = 0;
                }
                let start = start as usize;
                if n < 0 {
                    n = 0;
                }
                let n = n as usize;
                let mut end = start + n;
                if end > src.len() {
                    end = src.len();
                }
                let result = &src[start..end];
                let result = LString::from(result);
                Value::String(LRc::new(result))
            }
            replace => {
                let with = run.stack.pop().unwrap();
                let pat = run.stack.pop().unwrap();
                let src = run.stack.pop().unwrap();
                let result = src.string().replace(pat.string(), with.string());
                Value::String(LRc::new(result))
            }
            contains => {
                let pat = run.stack.pop().unwrap();
                let src = run.stack.pop().unwrap();
                let pat: &str = pat.string();
                let src: &str = src.string();
                let result = src.contains(pat);
                Value::Bool(result)
            }
            fn_text => {
                let fname = run.stack.pop().unwrap();
                let schema = run.stack.pop().unwrap();

                let sid = run.dict.schema_id(schema.string()).unwrap();
                let nameid = run.dict.name_id(fname.string()).unwrap();
                let fix = run.dict.func_index(&(*sid, *nameid)).unwrap();
                let func = run.dict.func_info(*fix);

                // println!( "FnText ... {:?}", func );

                let result = func.to_source(run.dict);

                Value::String(LRc::new(result))
            }
            table_text => {
                let tname = run.stack.pop().unwrap();
                let tname = tname.string();
                let schema = run.stack.pop().unwrap();
                let schema = schema.string();

                let sid = run.dict.schema_id(schema).unwrap();
                let nameid = run.dict.name_id(tname).unwrap();
                let (_, dt) = run.dict.table(&(*sid, *nameid)).unwrap();
                let mut result = LString::new();
                use std::fmt::Write;
                write!(&mut result, "table {}.{} {}", schema, tname, dt).unwrap();
                Value::String(LRc::new(result))
            }
            table_col_names => {
                let tname = run.stack.pop().unwrap();
                let tname = tname.string();
                let schema = run.stack.pop().unwrap();
                let schema = schema.string();

                let sid = run.dict.schema_id(schema).unwrap();
                let nameid = run.dict.name_id(tname).unwrap();
                let (_, dt) = run.dict.table(&(*sid, *nameid)).unwrap();
                let mut result = LString::new();
                let dt = dt.struc();
                for (i, (name, _)) in dt.iter().enumerate() {
                    if i != 0 {
                        result.push_str(", ");
                    }
                    result.push_str(name);
                }
                Value::String(LRc::new(result))
            }

            col_is_referenced => {
                let cname = run.stack.pop().unwrap();
                let cname = cname.string();
                let tname = run.stack.pop().unwrap();
                let tname = tname.string();
                let schema = run.stack.pop().unwrap();
                let schema = schema.string();

                let sid = run.dict.schema_id(schema).unwrap();
                let nameid = run.dict.name_id(tname).unwrap();
                let (tid, dt) = run.dict.table(&(*sid, *nameid)).unwrap();
                let cid = dt.lookup_col(cname).unwrap();
                let result = run.dict.col_is_referenced(tid, cid);
                Value::Bool(result)
            }

            table_literal => {
                let tname = run.stack.pop().unwrap();
                let tname = tname.string();
                let schema = run.stack.pop().unwrap();
                let schema = schema.string();

                let sid = run.dict.schema_id(schema).unwrap();
                let nameid = run.dict.name_id(tname).unwrap();
                let (_, dt) = run.dict.table(&(*sid, *nameid)).unwrap();
                let mut result = LString::new();
                let dt = dt.struc();
                use std::fmt::Write;
                for (i, (name, typ)) in dt.iter().enumerate() {
                    if i != 0 {
                        result.push_str(",',', ");
                    }
                    match typ {
                        DataType::String(_) => {
                            write!(&mut result, "sys.string_literal({})", name).unwrap()
                        }
                        _ => write!(&mut result, "{}", name).unwrap(),
                    }
                }
                Value::String(LRc::new(result))
            }
            string_literal => {
                let str = run.stack.pop().unwrap();
                let mut result = LString::new();
                str_literal(str.string(), &mut result);
                Value::String(LRc::new(result))
            }
            execute => {
                run.source = run.stack.pop().unwrap().string_clone();
                go(run);
                Value::Empty
            }
            batch => {
                let source = run.stack.pop().unwrap().string_clone();
                run.batch.push(source);
                Value::Empty
            }
            arg => {
                let name = run.stack.pop().unwrap();
                let kind = run.stack.pop().unwrap();
                let result = run.tr.arg(kind.int(), name.string());
                Value::String(result)
            }
            header => {
                let value = run.stack.pop().unwrap();
                let name = run.stack.pop().unwrap();
                run.tr.header(name.string(), value.string());
                Value::Empty
            }
            parseint => {
                let str = run.stack.pop().unwrap();
                let str = str.string();
                let i = str.parse::<i64>().unwrap_or_default();
                Value::Int(i)
            }
            error => Value::String(run.tr.get_error()),
        }
    }
    pub fn result_type(&self) -> &'static DataType {
        use Builtin::*;
        match self {
            execute | batch | header => &DataType::Empty,
            contains | col_is_referenced => &DataType::Bool,
            len | parseint => &DataType::Int,
            substr | replace | fn_text | table_text | arg | table_literal | table_col_names
            | string_literal | error => &DataType::String(0),
        }
    }

    pub fn arg_types(&self) -> &'static [DataType] {
        use Builtin::*;
        match self {
            len | string_literal => &STR_1,
            substr => &STR_INT_INT,
            replace | col_is_referenced => &STR_3,
            contains | header | fn_text | table_text | table_col_names | table_literal => &STR_2,
            execute | batch | parseint => &STR_1,
            arg => &INT_STR,
            error => &[],
        }
    }
}

const STR_1: [DataType; 1] = [DataType::String(0)];
const STR_2: [DataType; 2] = [DataType::String(0), DataType::String(0)];
const STR_3: [DataType; 3] = [
    DataType::String(0),
    DataType::String(0),
    DataType::String(0),
];
const STR_INT_INT: [DataType; 3] = [DataType::String(0), DataType::Int, DataType::Int];
const INT_STR: [DataType; 2] = [DataType::Int, DataType::String(0)];

pub fn str_literal(s: &str, to: &mut LString) {
    use std::fmt::Write;
    if !s.contains('\'') {
        write!(to, "'{}'", s).unwrap();
    } else if !s.contains('"') {
        write!(to, "\"{}\"", s).unwrap();
    } else {
        write!(to, "#s{}'{}'", s.len(), s).unwrap(); // Details and parsing for this are todo...
    }
}
