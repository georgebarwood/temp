use crate::*;

#[derive(
    Clone,
    Debug,
    Default,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
/// Generic value.
pub enum Value {
    #[default]
    Empty,
    Bool(bool),
    Int(i64),
    Float(F64),
    String(LRc<LString>),
    Binary(LRc<LVec<u8>>),
    List(LRc<LVec<Value>>),
    Enum(usize, LBox<Value>),
    IList(LRc<LVec<i64>>),
    DataType(DataType),
}

impl Value {
    /// Get bool ( Value must be Bool ).
    pub fn bool(&self) -> bool {
        match self {
            Value::Bool(x) => *x,
            _ => panic!("bool expected"),
        }
    }
    
    /// Get int ( Value must be Int ).
    pub fn int(&self) -> i64 {
        match self {
            Value::Int(x) => *x,
            _ => panic!("int expected"),
        }
    }

    /// Get float ( Value must be Int ).
    pub fn float(&self) -> f64 {
        match self {
            Value::Float(x) => x.0,
            _ => panic!("float expected"),
        }
    }

    /// Get reference to `LString` ( Value must be String )
    pub fn string(&self) -> &LString {
        match self {
            Value::String(s) => s,
            _ => panic!("string expected"),
        }
    }

    /// Get reference to `LVec<u8>` ( Value must be Binary )
    pub fn binary(&self) -> &LVec<u8> {
        match self {
            Value::Binary(b) => b,
            _ => panic!("binary expected"),
        }
    }

    /// Get reference to List LVec ( Value must be List )
    pub fn list(&self) -> &LRc<LVec<Value>> {
        match self {
            Value::List(list) => list,
            _ => panic!("list expected"),
        }
    }

    /// Get mut reference to List LVec ( Value must be List )
    pub fn list_mut(&mut self) -> &mut LRc<LVec<Value>> {
        match self {
            Value::List(list) => list,
            _ => panic!("list expected"),
        }
    }

    /// Get references to Enum tag and value ( Value must be Enum ).
    pub fn en(&self) -> (&usize, &Value) {
        match self {
            Value::Enum(tag, bx) => (tag, &**bx),
            _ => panic!("enum expected"),
        }
    }

    /// Get mut references to Enum tag and value ( Value must be Enum ).
    pub fn en_mut(&mut self) -> (&usize, &mut Value) {
        match self {
            Value::Enum(tag, bx) => (tag, &mut **bx),
            _ => panic!("enum expected"),
        }
    }

    /// Get reference to IList LVec ( Value must be IList ).
    pub fn ilist(&self) -> &LRc<LVec<i64>> {
        match self {
            Value::IList(list) => list,
            _ => panic!("ilist expected"),
        }
    }

    /// Get reference to IList LVec ( Value must be IList ).
    pub fn ilist_mut(&mut self) -> &mut LRc<LVec<i64>> {
        match self {
            Value::IList(list) => list,
            _ => panic!("ilist expected"),
        }
    }

    /// Get reference to DataType ( Value must be DataType ).
    pub fn datatype(&self) -> &DataType {
        match self {
            Value::DataType(dt) => dt,
            _ => panic!("datatype expected"),
        }
    }

    /// Get mut reference to DataType ( Value must be DataType ).
    pub fn datatype_mut(&mut self) -> &mut DataType {
        match self {
            Value::DataType(dt) => dt,
            _ => panic!("datatype expected"),
        }
    }
}

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct F64(pub f64);

impl Hash for F64 {
    fn hash<H>(&self, h: &mut H)
    where
        H: Hasher,
    {
        h.write(&self.0.to_le_bytes());
    }
}

impl Ord for F64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self < other {
            Ordering::Less
        } else if self > other {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

impl PartialOrd for F64 {
    fn partial_cmp(&self, other: &F64) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for F64 {}
