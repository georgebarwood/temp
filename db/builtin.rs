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
    execute,
    arg,
    header,
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
            b"execute" => Ok(execute),
            b"arg" => Ok(arg),
            b"header" => Ok(header),
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
            execute => {
                run.source = run.stack.pop().unwrap().string_clone();
                go(run);
                Value::Bool(true)
            }
            arg => {
                let name = run.stack.pop().unwrap();
                let kind = run.stack.pop().unwrap();
                let result = run.tr.arg( kind.int(), name.string() );
                Value::String(result)
            }
            header => {
                let value = run.stack.pop().unwrap();
                let name = run.stack.pop().unwrap();
                run.tr.header( name.string(), value.string() );
                Value::Bool(true)
            }
        }
    }
    pub fn result_type(&self) -> &'static DataType {
        use Builtin::*;
        match self {
            execute | contains | header => &DataType::Bool,
            len  => &DataType::Int,
            substr | replace | fn_text | arg => &DataType::String(0),
        }
    }

    pub fn arg_types(&self) -> &'static [DataType] {
        use Builtin::*;
        match self {
            len => &STR_1,
            substr => &STR_INT_INT,
            replace => &STR_3,
            contains | header => &STR_2,
            fn_text => &STR_2,
            execute => &STR_1,
            arg => &INT_STR,
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
