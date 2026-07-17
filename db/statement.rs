use crate::*;
use datatype::DataType;
use pstd::{VecA, alloc::Allocator};
use serde::*;

/// CREATE SCHEMA statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSchema {
    pub sname: StrPos,
}

/// CREATE TABLE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTable {
    pub schema_id: i64,
    pub tname: StrPos,
    pub col_defs: Arc<DataType>,
}

/// RENAME TABLE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameTable {
    pub old_schema_id: i64,
    pub old_nid: i64,
    pub new_schema_id: i64,
    pub new_tname: StrPos,
}

/// CREATE FN statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFn<A: Allocator + Default> {
    pub schema_id: i64,
    pub fname: StrPos,
    pub ret: Arc<DataType>,
    pub parms: VecA<(StrPos, Arc<DataType>),A>,
    pub block: VecA<Statement<A, YesString>,A>,
}

/// RENAME FN statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameFn {
    pub old_schema_id: i64,
    pub old_nid: i64,
    pub new_schema_id: i64,
    pub new_fname: StrPos,
}

/// DROP TABLE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropTable {
    pub schema_id: i64,
    pub name_id: i64,
    pub table: Arc<STable>,
}

/// LET statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrcLet<A: Allocator + Default> {
    pub varname: StrPos,
    pub exp: Exp<A>,
}

impl<A: Allocator + Default> SrcLet<A> {
    pub fn exec(&self, run: &mut Run) {
        let v = self.exp.eval(run);
        run.stack.push(v);
    }
}

/// LET statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Let<A: Allocator + Default, S: XString> {
    pub varname: S,
    pub exp: Exp<A>,
}

impl<A: Allocator + Default, S: XString> Let<A, S> {
    pub fn exec(&self, run: &mut Run) {
        let v = self.exp.eval(run);
        run.stack.push(v);
    }
}

/// SET statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Set<A: Allocator + Default> {
    pub i: usize,
    pub exp: Exp<A>,
}

impl<A: Allocator + Default> Set<A> {
    pub fn exec(&self, run: &mut Run) {
        let v = self.exp.eval(run);
        let ix = run.stack.len() - 1 - self.i;
        run.stack[ix] = v;
    }
}

/// APPEND ( |= ) statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Append<A: Allocator + Default> {
    pub i: usize,
    pub exp: Exp<A>,
}

impl<A: Allocator + Default> Append<A> {
    pub fn exec(&self, run: &mut Run) {
        let v = self.exp.eval(run);
        let ix = run.stack.len() - 1 - self.i;
        append(&mut run.stack[ix], &v);
    }
}

/// WHILE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct While<A: Allocator + Default, S: XString> {
    pub exp: Exp<A>,
    pub block: VecA<Statement<A, S>, A>,
}

impl<A: Allocator + Default, S: XString> While<A, S> {
    pub fn exec(&self, run: &mut Run) {
        while self.exp.eval(run).bool() {
            execute_block(&self.block, run);
        }
    }
}

/// IF statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct If<A: Allocator + Default, S: XString> {
    pub exp: Exp<A>,
    pub block: VecA<Statement<A, S>, A>,
    pub els: Option<VecA<Statement<A, S>, A>>,
}

impl<A: Allocator + Default, S: XString> If<A, S> {
    pub fn exec(&self, run: &mut Run) {
        if self.exp.eval(run).bool() {
            execute_block(&self.block, run);
        } else if let Some(els) = &self.els {
            execute_block(els, run);
        }
    }
}

/// INSERT statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insert<A: Allocator + Default> {
    pub table: Arc<STable>,
    pub cols: VecA<usize, A>,
    pub vals: VecA<Exp<A>, A>,
}

impl<A: Allocator + Default> Insert<A> {
    pub fn exec(&self, run: &mut Run) {
        // First evaluate the expressions.
        let mut ee = LVec::with_capacity(self.vals.len());
        for e in &self.vals {
            ee.push(e.eval(run));
        }
        let t = &self.table;
        let t = run.ps.load_table(t.id, &t.dt);
        let mut table = t.borrow_mut();

        let mut row = table.datatype.default_value();

        let list = row.list_mut();
        let mrow = LRc::make_mut(list);

        // Assign the columns, with the evaluated expressions.
        for (i, e) in ee.into_iter().enumerate() {
            let col = self.cols[i];
            mrow[col] = e;
        }

        let auto_id = !self.cols.contains(&0);
        let row_id = if auto_id {
            let row_id = table.new_id();
            mrow[0] = Value::Int(row_id); // Assign the id to the first element.
            row_id
        } else {
            let row_id = mrow[0].int();
            table.reserve_id(row_id);
            row_id
        };

        if !auto_id {
            table.remove(row_id, run.ps); // Remove any existing record before inserting.
        }

        table.insert(&row, run.ps);

        println!(
            "Insert exec table record count={} row={:?}",
            table.record_count(),
            row
        );
    }
}

/// UPDATE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update<A: Allocator + Default> {
    pub table: Arc<STable>,
    pub assigns: VecA<(usize, Exp<A>), A>, // col num, Exp
    pub wher: Exp<A>,
}

impl<A: Allocator + Default> Update<A> {
    pub fn exec(&self, run: &mut Run) {
        let t = run.ps.load_table(self.table.id, &self.table.dt);
        let ids = ids(&t, &self.wher, run);
        let mut table = t.borrow_mut();
        for id in &ids {
            let mut row = table.fetch(*id, run.ps).unwrap();
            let mut vals = LVec::with_capacity(self.assigns.len());
            {
                for (_col, e) in &self.assigns {
                    let v = e.eval_vals(run, row.list());
                    vals.push(v);
                }
            }
            let mrow = LRc::make_mut(row.list_mut());
            for (col, _e) in self.assigns.iter().rev() {
                mrow[*col] = vals.pop().unwrap();
            }
            table.update(*id, &row, run.ps);
        }
    }
}

/// DELETE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delete<A: Allocator + Default> {
    pub table: Arc<STable>,
    pub wher: Exp<A>,
}

impl<A: Allocator + Default> Delete<A> {
    pub fn exec(&self, run: &mut Run) {
        let t = run.ps.load_table(self.table.id, &self.table.dt);
        let ids = ids(&t, &self.wher, run);
        let mut table = t.borrow_mut();
        for id in &ids {
            table.remove(*id, run.ps);
        }
    }
}

pub type OrderBy<A> = Option<(VecA<Exp<A>, A>, VecA<bool, A>)>;

/// SELECT statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Select<A: Allocator + Default> {
    pub vals: VecA<Exp<A>, A>,
    pub from: Option<Arc<STable>>,
    pub wher: Option<Exp<A>>,
    pub order_by: OrderBy<A>,
}

impl<A: Allocator + Default> Select<A> {
    pub fn exec(&self, run: &mut Run) {
        if self.order_by.is_some() {
            self.exec_order_by(run)
        } else if let Some(f) = &self.from {
            let t = run.ps.load_table(f.id, &f.dt);
            let table = t.borrow();
            let mut iter = table.iter(run.ps);
            while let Some(b) = iter.next_ref(run.ps) {
                // print!("got a row :");
                let mut lr = table.lazy_row(b);
                let ok = if let Some(wher) = &self.wher {
                    wher.eval_lr(run, &mut lr).bool()
                } else {
                    true
                };
                if ok {
                    for e in &self.vals {
                        let v = e.eval_lr(run, &mut lr);
                        run.output(&v);
                    }
                }
            }
        } else {
            // SELECT with no FROM
            for e in &self.vals {
                let v = e.eval(run);
                run.output(&v);
            }
        }
    }
    pub fn exec_order_by(&self, run: &mut Run) {
        let f = self.from.as_ref().unwrap();
        let temp = get_temp(f, &self.vals, &self.wher, &self.order_by, run);

        let n = self.order_by.as_ref().unwrap().0.len();
        for row in &temp {
            for v in &row[n..] {
                run.output(v);
            }
        }
    }
}

/// FOR statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct For<A: Allocator + Default, S: XString> {
    pub vals: VecA<Exp<A>, A>,
    pub from: Arc<STable>,
    pub wher: Option<Exp<A>>,
    pub order_by: OrderBy<A>,
    pub block: VecA<Statement<A, S>, A>,
}

impl<A: Allocator + Default, S: XString> For<A, S> {
    pub fn exec(&self, run: &mut Run) {
        if self.order_by.is_some() {
            self.exec_order_by(run);
        } else {
            let t = run.ps.load_table(self.from.id, &self.from.dt);
            let table = t.borrow();
            let mut iter = table.iter(run.ps);
            while let Some(b) = iter.next_ref(run.ps) {
                let mut lr = table.lazy_row(b);

                let ok = if let Some(wher) = &self.wher {
                    let v = wher.eval_lr(run, &mut lr);
                    v.bool()
                } else {
                    true
                };

                if ok {
                    let len = run.stack.len();
                    for e in &self.vals {
                        let v = e.eval_lr(run, &mut lr);
                        run.stack.push(v);
                    }
                    execute_block(&self.block, run);
                    run.stack.truncate(len);
                }
            }
        }
    }
    pub fn exec_order_by(&self, run: &mut Run) {
        let temp = get_temp(&self.from, &self.vals, &self.wher, &self.order_by, run);

        let n = self.order_by.as_ref().unwrap().0.len();

        for row in &temp {
            let len = run.stack.len();
            for v in &row[n..] {
                run.stack.push(v.clone());
            }
            execute_block(&self.block, run);
            run.stack.truncate(len);
        }
    }
}

/// Statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Statement<A: Allocator + Default, S: XString> {
    /// Declare and initialise a local variable.
    SrcLet(SrcLet<A>),
    /// Declare and initialise a local variable.
    Let(Let<A, S>),
    /// Assign a local variable.
    Set(Set<A>),
    /// Append to a local string or binary variable.
    Append(Append<A>),
    /// While loop.
    While(While<A, S>),
    /// Conditional evalaution.
    If(If<A, S>),
    /// Insert into table.
    Insert(Insert<A>),
    /// Update table rows. Where condition is not optional, use "where true" to update all rows.
    Update(Update<A>),
    /// Delete rows from table. Where condition is not optional, use "where true" to delete all rows.
    Delete(Delete<A>),
    /// Output values.
    Select(Select<A>),
    /// Loop through table, local variables are assigned to expressions evaluated from table rows.
    For(For<A, S>),
    /// Create Schema.
    CreateSchema(CreateSchema),
    /// Create Table.
    CreateTable(CreateTable),
    /// Rename Table.
    RenameTable(RenameTable),
    /// Create Function.
    CreateFn(CreateFn<A>),
    /// Rename Function.
    RenameFn(RenameFn),
    /// Drop Table.
    DropTable(DropTable),
}

impl<A, S> Statement<A, S>
where
    A: Allocator + Default,
    S: XString,
{
    fn from(stat: &LStatement, src: &[u8]) -> Self {
        match stat {
            Statement::SrcLet(x) => {
                let varname = x.varname.str(src);
                Statement::Let(Let {
                    varname: S::from_str(varname),
                    exp: Exp::from(&x.exp, src),
                })
            }
            Statement::Let(x) => Statement::Let(Let {
                varname: S::from_str(x.varname.str()),
                exp: Exp::from(&x.exp, src),
            }),
            Statement::Set(x) => Statement::Set(Set {
                i: x.i,
                exp: Exp::from(&x.exp, src),
            }),
            Statement::Append(x) => Statement::Append(Append {
                i: x.i,
                exp: Exp::from(&x.exp, src),
            }),
            Statement::While(x) => {
                let exp = Exp::from(&x.exp, src);
                let block = gblock(&x.block, src);
                Statement::While(While { exp, block })
            }
            Statement::If(x) => {
                let exp = Exp::from(&x.exp, src);
                let block = gblock(&x.block, src);
                let els = x.els.as_ref().map(|els| gblock(els, src));
                Statement::If(If { exp, block, els })
            }
            Statement::Insert(x) => {
                let table = x.table.clone();
                let cols = VecA::from(&*x.cols);
                let vals = gvals(&x.vals, src);
                Statement::Insert(Insert { table, cols, vals })
            }
            Statement::Select(x) => {
                let vals = gvals(&x.vals, src);
                let from = x.from.clone();
                let wher = x.wher.as_ref().map(|wher| Exp::from(wher, src));
                let order_by = gorder_by(&x.order_by, src);
                Statement::Select(Select {
                    vals,
                    from,
                    wher,
                    order_by,
                })
            }
            Statement::For(x) => {
                let vals = gvals(&x.vals, src);
                let from = x.from.clone();
                let wher = x.wher.as_ref().map(|wher| Exp::from(wher, src));
                let order_by = gorder_by(&x.order_by, src);
                let block = gblock(&x.block, src);
                Statement::For(For {
                    vals,
                    from,
                    wher,
                    order_by,
                    block,
                })
            }
            Statement::Update(x) => {
                let table = x.table.clone();
                let wher = Exp::from(&x.wher, src);
                let mut assigns = VecA::new();
                for (i, e) in &x.assigns {
                    assigns.push((*i, Exp::from(e, src)));
                }
                Statement::Update(Update {
                    table,
                    assigns,
                    wher,
                })
            }
            Statement::Delete(x) => {
                let table = x.table.clone();
                let wher = Exp::from(&x.wher, src);
                Statement::Delete(Delete { table, wher })
            }
            _ => panic!(),
        }
    }
}

pub fn gblock<A, S>(list: &[LStatement], src: &[u8]) -> VecA<Statement<A, S>, A>
where
    A: Allocator + Default,
    S: XString,
{
    let mut block = VecA::with_capacity(list.len());
    for s in list {
        block.push(Statement::from(s, src));
    }
    block
}

pub fn execute_fn<S>(f: &SFunc<S>, run: &mut Run)
where
    S: XString,
{
    // println!("execute_fn f={:?}", f);
    execute_block(&f.block, run);
}

pub fn execute_block<A, S>(slist: &[Statement<A, S>], run: &mut Run)
where
    A: Allocator + Default,
    S: XString,
{
    let slen = run.stack.len(); // At end restore stack to this length.
    for s in slist {
        use Statement::*;
        match s {
            SrcLet(x) => x.exec(run),
            Let(x) => x.exec(run),
            Set(x) => x.exec(run),
            Append(x) => x.exec(run),
            While(x) => x.exec(run),
            If(x) => x.exec(run),
            Insert(x) => x.exec(run),
            Update(x) => x.exec(run),
            Delete(x) => x.exec(run),
            Select(x) => x.exec(run),
            For(x) => x.exec(run),
            CreateSchema(_) | CreateTable(_) | RenameTable(_) | CreateFn(_) | RenameFn(_)
            | DropTable(_) => panic!(),
        };
    }
    run.stack.truncate(slen); // pop local variables from stack.
}

/// Get a list of ids for records from table that satisfy where condition.
fn ids<A>(t: &RTable, wher: &Exp<A>, run: &mut Run) -> LVec<i64>
where
    A: Allocator + Default,
{
    let mut result = LVec::new();
    let table = t.borrow();
    let mut iter = table.iter(run.ps);
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        if wher.eval_lr(run, &mut lr).bool() {
            let id = lr.item(0, run.ps).int();
            result.push(id);
        }
    }
    result
}

pub fn gvals<A>(list: &[LExp], src: &[u8]) -> VecA<Exp<A>, A>
where
    A: Allocator + Default,
{
    let mut result = VecA::with_capacity(list.len());
    for e in list {
        result.push(Exp::from(e, src));
    }
    result
}

fn gorder_by<A>(list: &LOrderBy, src: &[u8]) -> OrderBy<A>
where
    A: Allocator + Default,
{
    if let Some((exps, descs)) = list {
        let mut result = VecA::with_capacity(exps.len());
        for e in exps {
            result.push(Exp::from(e, src));
        }
        let descs = VecA::from(&**descs);
        Some((result, descs))
    } else {
        None
    }
}

fn get_temp<A>(
    st: &STable,
    vals: &[Exp<A>],
    wher: &Option<Exp<A>>,
    order_by: &OrderBy<A>,
    run: &mut Run,
) -> LVec<LVec<Value>>
where
    A: Allocator + Default,
{
    let (ob, desc) = order_by.as_ref().unwrap();
    let table = run.ps.load_table(st.id, &st.dt);
    let table = table.borrow();
    let mut iter = table.iter(run.ps);

    let mut temp = LVec::new();
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        let ok = if let Some(wher) = &wher {
            wher.eval_lr(run, &mut lr).bool()
        } else {
            true
        };
        if ok {
            let mut row = LVec::with_capacity(ob.len() + vals.len());
            for e in ob {
                let v = e.eval_lr(run, &mut lr);
                row.push(v);
            }
            for e in vals {
                let v = e.eval_lr(run, &mut lr);
                row.push(v);
            }
            temp.push(row);
        }
    }
    temp.sort_by(|a, b| row_compare(a, b, desc));
    temp
}

use std::fmt::Debug;
pub trait XString {
    fn str(&self) -> &str;
    fn from_str(s: &str) -> Self;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YesString {
    s: GString,
}

impl XString for YesString {
    fn str(&self) -> &str {
        &self.s
    }
    fn from_str(s: &str) -> Self {
        Self {
            s: GString::from(s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoString {}

impl XString for NoString {
    fn str(&self) -> &str {
        ""
    }
    fn from_str(_s: &str) -> Self {
        Self {}
    }
}

pub type LStatement = Statement<Local, YesString>;
pub type LOrderBy = OrderBy<Local>;
pub type LExp = Exp<Local>;
