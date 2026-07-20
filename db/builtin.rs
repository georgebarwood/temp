use crate::*;
use serde::*;

/// Builtin functions
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Builtin {
    Len,
    Substr,
    Replace,
    Contains,
    BinLen,
    BinSubstr,
    FnText,
    // More to do...
}

impl Builtin {
    pub fn new(name: &[u8]) -> Result<Self, E> {
        use Builtin::*;
        match name {
            b"Len" => Ok(Len),
            b"Substr" => Ok(Substr),
            b"Replace" => Ok(Replace),
            b"Fn_text" => Ok(FnText),
            _ => Err(E::new("Unknown sys call")),
        }
    }

    pub fn eval(&self, run: &mut Run) -> Value {
        // Arguments are on stack
        use Builtin::*;
        match self {
            Len => {
                let s = run.stack.pop().unwrap();
                Value::Int(s.string().len() as i64)
            }
            Substr => {
                let mut len = run.stack.pop().unwrap().int();
                let mut start = run.stack.pop().unwrap().int();
                let src = run.stack.pop().unwrap();
                let src = src.string();
                if start < 0 {
                    start = 0;
                }
                let start = start as usize;
                if len < 0 {
                    len = 0;
                }
                let len = len as usize;
                let mut end = start + len;
                if end > src.len() {
                    end = src.len();
                }
                let result = &src[start..end];
                let result = LString::from(result);
                Value::String(LRc::new(result))
            }
            Replace => {
                let with = run.stack.pop().unwrap();
                let pat = run.stack.pop().unwrap();
                let src = run.stack.pop().unwrap();
                let result = src.string().replace(pat.string(), with.string());
                Value::String(LRc::new(result))
            }
            FnText => {
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
            _ => todo!(),
        }
    }
    pub fn result_type(&self) -> &'static DataType {
        use Builtin::*;
        match self {
            Len => &DataType::Int,
            Substr => &DataType::String(0),
            Replace => &DataType::String(0),
            FnText => &DataType::String(0),
            _ => todo!(),
        }
    }

    pub fn arg_types(&self) -> &'static [DataType] {
        use Builtin::*;
        match self {
            Len => &STR_1,
            Substr => &STR_INT_INT,
            Replace => &STR_3,
            FnText => &STR_2,
            _ => todo!(),
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
