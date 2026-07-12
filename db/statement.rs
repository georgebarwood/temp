use crate::*;
use datatype::DataType;
use serde::*;

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

/// CREATE FN statement.
#[derive(Debug)]
pub struct CreateFn<'a> {
    pub schema_id: i64,
    pub fname: &'a str,
    pub rtyp: Arc<DataType>,
    pub args: LVec< (&'a str, Arc<DataType>) >,
    pub block: LVec<(usize, Statement<'a>)>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct GInsert {
    pub table: Arc<STable>,
    pub cols: GVec<usize>,
    pub vals: GVec<GExp>,
}

/// SELECT statement.
#[derive(Debug)]
pub struct Select<'a> {
    pub vals: LVec<Exp<'a>>,
    pub from: Option<Arc<STable>>,
    pub wher: Option<Exp<'a>>,
    pub order_by: Option<LVec<(Exp<'a>, bool)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GSelect {
    pub vals: GVec<GExp>,
    pub from: Option<Arc<STable>>,
    pub wher: Option<GExp>,
    pub order_by: Option<LVec<(GExp, bool)>>,
}

/// FOR statement.
#[derive(Debug)]
pub struct For<'a> {
    pub vals: LVec<Exp<'a>>,
    pub from: Arc<STable>,
    pub wher: Option<Exp<'a>>,
    pub order_by: Option<LVec<(Exp<'a>, bool)>>,
    pub block: LVec<(usize, Statement<'a>)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GFor {
    pub vals: GVec<GExp>,
    pub from: Arc<STable>,
    pub wher: Option<GExp>,
    pub order_by: Option<GVec<(GExp, bool)>>,
    pub block: GVec<(usize, GStatement)>,
}

/// UPDATE statement.
#[derive(Debug)]
pub struct Update<'a> {
    pub assigns: LVec<(usize, Exp<'a>)>, // col num, Exp
    pub table: Arc<STable>,
    pub wher: Exp<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GUpdate {
    pub assigns: GVec<(usize, GExp)>, // col num, Exp
    pub table: Arc<STable>,
    pub wher: GExp,
}

/// DELETE statement.
#[derive(Debug)]
pub struct Delete<'a> {
    pub table: Arc<STable>,
    pub wher: Exp<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GDelete {
    pub table: Arc<STable>,
    pub wher: GExp,
}

/// LET statement.
#[derive(Debug)]
pub struct Let<'a> {
    pub exp: Exp<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GLet {
    pub exp: GExp,
}

/// SET statement.
#[derive(Debug)]
pub struct Set<'a> {
    pub i: usize,
    pub exp: Exp<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GSet {
    pub i: usize,
    pub exp: GExp,
}

/// WHILE statement.
#[derive(Debug)]
pub struct While<'a> {
    pub exp: Exp<'a>,
    pub block: LVec<(usize, Statement<'a>)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GWhile {
    pub exp: GExp,
    pub block: GVec<(usize, GStatement)>,
}

/// IF statement.
#[derive(Debug)]
pub struct If<'a> {
    pub exp: Exp<'a>,
    pub block: LVec<(usize, Statement<'a>)>,
    pub els: Option<LVec<(usize, Statement<'a>)>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GIf {
    pub exp: GExp,
    pub block: GVec<(usize, GStatement)>,
    pub els: Option<GVec<(usize, GStatement)>>,
}

/// Statement.
#[derive(Debug)]
pub enum Statement<'a> {
    Let(Let<'a>),
    Set(Set<'a>),
    While(While<'a>),
    If(If<'a>),

    Insert(Insert<'a>),
    Select(Select<'a>),
    For(For<'a>),
    Update(Update<'a>),
    Delete(Delete<'a>),

    CreateSchema(CreateSchema<'a>),
    CreateTable(CreateTable<'a>),
    CreateFn(CreateFn<'a>),
    DropTable(DropTable),
}

// Similar to Statement but storeable and shareable.
#[derive(Debug, Serialize, Deserialize)]
pub enum GStatement {
    Let(GLet),
    Set(GSet),
    While(GWhile),
    If(GIf),
    Insert(GInsert),
    Select(GSelect),
    For(GFor),
    Update(GUpdate),
    Delete(GDelete),
}

impl GStatement {
    pub fn from(stat: &Statement) -> Self {
        match stat {
            Statement::Let(x) => GStatement::Let(GLet {
                exp: GExp::from(&x.exp),
            }),
            Statement::Set(x) => GStatement::Set(GSet {
                i: x.i,
                exp: GExp::from(&x.exp),
            }),
            Statement::While(x) => {
                let exp = GExp::from(&x.exp);
                let block = gblock(&x.block);
                GStatement::While(GWhile { exp, block })
            }
            Statement::If(x) => {
                let exp = GExp::from(&x.exp);
                let block = gblock(&x.block);
                let els = x.els.as_ref().map(|els| gblock(els));
                GStatement::If(GIf { exp, block, els })
            }
            Statement::Insert(x) => {
                let table = x.table.clone();
                let cols = GVec::from(&*x.cols);
                let vals = gvals(&x.vals);
                GStatement::Insert(GInsert { table, cols, vals })
            }
            Statement::Select(x) => {
                let vals = gvals(&x.vals);
                let from = x.from.clone();
                let wher = x.wher.as_ref().map(|wher| GExp::from(wher));
                let order_by = None;
                GStatement::Select(GSelect {
                    vals,
                    from,
                    wher,
                    order_by,
                })
            }
            Statement::For(x) => {
                let vals = gvals(&x.vals);
                let from = x.from.clone();
                let wher = x.wher.as_ref().map(|wher| GExp::from(wher));
                let order_by = None;
                let block = gblock(&x.block);
                GStatement::For(GFor {
                    vals,
                    from,
                    wher,
                    order_by,
                    block,
                })
            }
            Statement::Update(x) => {
                let table = x.table.clone();
                let wher = GExp::from(&x.wher);
                let mut assigns = GVec::new();
                for (i, e) in &x.assigns {
                    assigns.push((*i, GExp::from(e)));
                }
                GStatement::Update(GUpdate {
                    table,
                    assigns,
                    wher,
                })
            }
            Statement::Delete(x) => {
                let table = x.table.clone();
                let wher = GExp::from(&x.wher);
                GStatement::Delete(GDelete { table, wher })
            }
            _ => panic!(),
        }
    }
}

fn gvals(list: &[Exp]) -> GVec<GExp>
{
    let mut result = GVec::with_capacity(list.len());
    for e in list {
       result.push( GExp::from(e) );
    }
    result
}

fn gblock(list: &[(usize, Statement)]) -> GVec<(usize, GStatement)> {
    let mut block = GVec::with_capacity(list.len());
    for (i, s) in list {
        block.push((*i, GStatement::from(s)));
    }
    block
}
