use crate::DICT_ID;
use datatype::DataType;
use tablestg::*;

use serde::*;
use std::collections::HashMap;

/// Dictionary to look up schema, tables, etc.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Dict {
    pub schemas: HashMap<GString, i64>,
    pub names: HashMap<GString, i64>,
    pub tables: HashMap<(i64, i64), Arc<STable>>,
    last_schema_id: i64,
    last_name_id: i64,
    last_table_id: i64,
}

impl Dict {
    pub fn new() -> Self {
        Self {
            last_table_id: DICT_ID as i64,
            ..Default::default()
        }
    }
    pub fn new_schema_id(&mut self) -> i64 {
        self.last_schema_id += 1;
        self.last_schema_id
    }
    pub fn new_table_id(&mut self) -> i64 {
        self.last_table_id += 1;
        self.last_table_id
    }
    pub fn new_name_id(&mut self, s: &str) -> i64 {
        if let Some(id) = self.names.get(s) {
            return *id;
        }
        self.last_name_id += 1;
        let id = self.last_name_id;
        self.names.insert(GString::from(s), id);
        id
    }

    /// Serialize as bytes, with pre-pended id.
    fn to_bytes_id(&self, id: u64) -> LVec<u8> {
        let mut result = LVec::new();
        result.extend_from_slice(&id.to_le_bytes());
        postcard::to_io(self, &mut result).unwrap();
        result
    }

    /// Deserialise from bytes, first 8 bytes are skipped (id field).
    fn from_bytes_id(b: &[u8]) -> Self {
        postcard::from_bytes(&b[8..]).unwrap()
    }

    /// Save dict to sys store.
    pub fn save_to_sys_store(&self, ps: &mut PageSet) {
        let id = crate::DICT_ID;
        let bytes = self.to_bytes_id(id);
        let ssc = ps.sys_store.clone();
        let mut sys_store = ssc.borrow_mut();
        let key = IdVKey::new(id);
        sys_store.replace(&key, &bytes, ps);
    }

    /// Load dict from sys store.
    pub fn load_from_sys_store(ps: &mut PageSet) -> Arc<Dict> {
        let ssc = ps.sys_store.clone();
        let sys_store = ssc.borrow();
        let key = IdVKey::new(crate::DICT_ID);
        if let Some(mut sdata) = sys_store.get(&key, ps) {
            let bytes = sdata.bytes();
            let dict = Dict::from_bytes_id(&bytes);
            Arc::new(dict)
        } else {
            panic!()
        }
    }
}

/// Schema Table - id and DataType.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct STable {
    pub id: i64,
    pub dt: Arc<DataType>,
}

impl STable {
    pub fn name_to_col(&self, s: &str) -> Option<(usize, &DataType)> {
        self.dt.name_to_col(s)
    }
}

/// Expression.
#[derive(Debug)]
pub enum Exp<'a> {
    /// Integer constant
    Int(i64),
    /// String literal
    String(&'a str),
    /// Unresolved name
    Name(&'a str),
    /// Column number
    Col(usize),
    /// Binary expression, e.g. Age + 10
    Binary(Operator, LBox<Exp<'a>>, LBox<Exp<'a>>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Operator {
    None,
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
    Plus,
    Minus,
    Multiply,
    Divide,
    Concat,
}

impl<'a> Exp<'a> {
    pub fn eval(&self) -> Value {
        match self {
            Exp::String(s) => Value::String(LRc::new(LString::from(*s))),
            Exp::Int(i) => Value::Int(*i),
            Exp::Binary(_op, lhs, rhs) => {
                let x = lhs.eval().int();
                let y = rhs.eval().int();
                Value::Int(x + y)
            }
            _ => todo!(),
        }
    }
    pub fn eval_from_row(&self, row: &mut LazyRow, ps: &mut PageSet) -> Value {
        match self {
            Exp::String(s) => Value::String(LRc::new(LString::from(*s))),
            Exp::Int(i) => Value::Int(*i),
            Exp::Col(i) => row.item(*i, ps),
            Exp::Binary(op, lhs, rhs) => {
                let x = lhs.eval_from_row(row, ps).int();
                let y = rhs.eval_from_row(row, ps).int();
                match op {
                   Operator::Plus => Value::Int(x + y),
                   Operator::Equal => Value::Bool(x == y),
                   _ => todo!()
                }
            }
            _ => {
                println!("todo: {:?}", self);
                panic!();
            }
        }
    }
}

/// Statement.
#[derive(Debug)]
pub enum Statement<'a> {
    CreateSchema(CreateSchema<'a>),
    CreateTable(CreateTable<'a>),
    DropTable(DropTable),
    Insert(Insert<'a>),
    Select(Select<'a>),
}

/// CREATE SCHEMA statement.
#[derive(Debug)]
pub struct CreateSchema<'a> {
    pub sname: &'a str,
}

/// CREATE TABLE statement.
#[derive(Debug)]
pub struct CreateTable<'a> {
    pub schema_id: i64,
    pub tname: &'a str,
    pub col_defs: Arc<DataType>,
}

/// DROP TABLE statement.
#[derive(Debug)]
pub struct DropTable {
    pub schema_id: i64,
    pub name_id: i64,
    pub table: Arc<STable>,
}

/// INSERT statement.
#[derive(Debug)]
pub struct Insert<'a> {
    pub table: Arc<STable>,
    pub cols: LVec<usize>,
    pub vals: LVec<Exp<'a>>,
}

/// SELECT statement.
#[derive(Debug)]
pub struct Select<'a> {
    pub vals: LVec<Exp<'a>>,
    pub from: Arc<STable>,
    pub wher: Option<Exp<'a>>,
    pub order_by: Option<LVec<Exp<'a>>>,
}
