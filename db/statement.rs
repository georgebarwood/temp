use crate::*;
use datatype::DataType;
use serde::*;

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
    pub ret: Arc<DataType>,
    pub args: LVec<(&'a str, Arc<DataType>)>,
    pub block: LVec<Statement<'a>>,
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

/// INSERT statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GInsert {
    pub table: Arc<STable>,
    pub cols: GVec<usize>,
    pub vals: GVec<GExp>,
}

pub type OrderBy<'a> = Option<(LVec<Exp<'a>>, LVec<bool>)>;
pub type GOrderBy = Option<(GVec<GExp>, GVec<bool>)>;

/// SELECT statement.
#[derive(Debug)]
pub struct Select<'a> {
    pub vals: LVec<Exp<'a>>,
    pub from: Option<Arc<STable>>,
    pub wher: Option<Exp<'a>>,
    pub order_by: OrderBy<'a>,
}

/// SELECT statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GSelect {
    pub vals: GVec<GExp>,
    pub from: Option<Arc<STable>>,
    pub wher: Option<GExp>,
    pub order_by: GOrderBy,
}

/// FOR statement.
#[derive(Debug)]
pub struct For<'a> {
    pub vals: LVec<Exp<'a>>,
    pub from: Arc<STable>,
    pub wher: Option<Exp<'a>>,
    pub order_by: OrderBy<'a>,
    pub block: LVec<Statement<'a>>,
}

/// FOR statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GFor {
    pub vals: GVec<GExp>,
    pub from: Arc<STable>,
    pub wher: Option<GExp>,
    pub order_by: GOrderBy,
    pub block: GVec<GStatement>,
}

/// UPDATE statement.
#[derive(Debug)]
pub struct Update<'a> {
    pub assigns: LVec<(usize, Exp<'a>)>, // col num, Exp
    pub table: Arc<STable>,
    pub wher: Exp<'a>,
}

/// UPDATE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// DELETE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDelete {
    pub table: Arc<STable>,
    pub wher: GExp,
}

/// LET statement.
#[derive(Debug)]
pub struct Let<'a> {
    pub exp: Exp<'a>,
}

/// LET statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GLet {
    pub exp: GExp,
}

/// SET statement.
#[derive(Debug)]
pub struct Set<'a> {
    pub i: usize,
    pub exp: Exp<'a>,
}

/// SET statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GSet {
    pub i: usize,
    pub exp: GExp,
}

/// APPEND ( |= ) statement.
#[derive(Debug)]
pub struct Append<'a> {
    pub i: usize,
    pub exp: Exp<'a>,
}

/// APPEND ( |= ) statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GAppend {
    pub i: usize,
    pub exp: GExp,
}

/// WHILE statement.
#[derive(Debug)]
pub struct While<'a> {
    pub exp: Exp<'a>,
    pub block: LVec<Statement<'a>>,
}

/// WHILE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GWhile {
    pub exp: GExp,
    pub block: GVec<GStatement>,
}

/// IF statement.
#[derive(Debug)]
pub struct If<'a> {
    pub exp: Exp<'a>,
    pub block: LVec<Statement<'a>>,
    pub els: Option<LVec<Statement<'a>>>,
}

/// IF statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GIf {
    pub exp: GExp,
    pub block: GVec<GStatement>,
    pub els: Option<GVec<GStatement>>,
}

/// Statement.
#[derive(Debug)]
pub enum Statement<'a> {
    /// Declare and initialise a local variable.
    Let(Let<'a>),
    /// Assign a local variable.
    Set(Set<'a>),
    /// Append to a local string or binary variable.
    Append(Append<'a>),
    /// While loop.
    While(While<'a>),
    /// Conditional evalaution.
    If(If<'a>),
    /// Insert into table.
    Insert(Insert<'a>),
    /// Output values.
    Select(Select<'a>),
    /// Loop through table, local variables are assigned to expressions evaluated from table rows.
    For(For<'a>),
    /// Update table rows. Where condition is not optional, use "where true" to update all rows.
    Update(Update<'a>),
    /// Delete rows from table. Where condition is not optional, use "where true" to delete all rows.
    Delete(Delete<'a>),
    /// Create Schema.
    CreateSchema(CreateSchema<'a>),
    /// Create Table.
    CreateTable(CreateTable<'a>),
    /// Create Function.
    CreateFn(CreateFn<'a>),
    /// Drop Table.
    DropTable(DropTable),
}

/// Similar to [Statement] but storeable and shareable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GStatement {
    /// Declare and initialise a local variable.
    Let(GLet),
    /// Assign a local variable.
    Set(GSet),
    /// Append to a local string or binary variable.
    Append(GAppend),
    /// While loop.
    While(GWhile),
    /// Conditional evalaution.
    If(GIf),
    /// Insert into table.
    Insert(GInsert),
    /// Output values.
    Select(GSelect),
    /// Loop through table, local variables are assigned to expressions evaluated from table rows.
    For(GFor),
    /// Update table rows. Where condition is not optional, use "where true" to update all rows.
    Update(GUpdate),
    /// Delete rows from table. Where condition is not optional, use "where true" to delete all rows.
    Delete(GDelete),
}

impl GStatement {
    /// Convert [Statement] to [GStatement].
    pub fn from(stat: &Statement) -> Self {
        match stat {
            Statement::Let(x) => GStatement::Let(GLet {
                exp: GExp::from(&x.exp),
            }),
            Statement::Set(x) => GStatement::Set(GSet {
                i: x.i,
                exp: GExp::from(&x.exp),
            }),
            Statement::Append(x) => GStatement::Append(GAppend {
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
                let order_by = gorder_by(&x.order_by);
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
                let order_by = gorder_by(&x.order_by);
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

/// Convert list of Exp to list of GExp.
pub fn gvals(list: &[Exp]) -> GVec<GExp> {
    let mut result = GVec::with_capacity(list.len());
    for e in list {
        result.push(GExp::from(e));
    }
    result
}

/// Convert list of Statements to list of GStatement.
pub fn gblock(list: &[Statement]) -> GVec<GStatement> {
    let mut block = GVec::with_capacity(list.len());
    for s in list {
        block.push(GStatement::from(s));
    }
    block
}

pub fn gorder_by(list: &OrderBy) -> GOrderBy {
    if let Some((exps, descs)) = list {
        let mut result = GVec::with_capacity(exps.len());
        for e in exps {
            result.push(GExp::from(e));
        }
        let descs = GVec::from(&**descs);
        Some((result, descs))
    } else {
        None
    }
}
