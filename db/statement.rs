use crate::*;
use datatype::DataType;

// Statements.

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
    pub from: Option<Arc<STable>>,
    pub wher: Option<Exp<'a>>,
    pub order_by: Option<LVec<(Exp<'a>, bool)>>,
}

/// UPDATE statement.
#[derive(Debug)]
pub struct Update<'a> {
    pub assigns: LVec<(usize, Exp<'a>)>, // col num, Exp
    pub table: Arc<STable>,
    pub wher: Exp<'a>,
}

/// DELETE statement.
#[derive(Debug)]
pub struct Delete<'a> {
    pub table: Arc<STable>,
    pub wher: Exp<'a>,
}

/// LET statement.
#[derive(Debug)]
pub struct Let<'a> {
    pub exp: Exp<'a>,
}

/// Set statement.
#[derive(Debug)]
pub struct Set<'a> {
    pub i: usize,
    pub exp: Exp<'a>,
}

/// WHILE statement.
#[derive(Debug)]
pub struct Whil<'a> {
    pub exp: Exp<'a>,
    pub block: LVec<(usize, Statement<'a>)>,
}

/// Statement.
#[derive(Debug)]
pub enum Statement<'a> {
    CreateSchema(CreateSchema<'a>),
    CreateTable(CreateTable<'a>),
    DropTable(DropTable),
    Insert(Insert<'a>),
    Select(Select<'a>),
    Update(Update<'a>),
    Delete(Delete<'a>),
    Let(Let<'a>),
    Set(Set<'a>),
    Whil(Whil<'a>),
}
