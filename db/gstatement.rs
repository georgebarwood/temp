use crate::*;
use serde::*;

/// LET statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GLet<S: XString> {
    pub varname: S, // Needed to be able to reconstruct source of stored function.
    pub exp: GExp,
}

impl<S: XString> GLet<S> {
    pub fn exec(&self, run: &mut Run) {
        let v = self.exp.eval(run);
        run.stack.push(v);
    }
}

/// SET statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GSet {
    pub i: usize,
    pub exp: GExp,
}

impl GSet {
    pub fn exec(&self, run: &mut Run) {
        let v = self.exp.eval(run);
        let ix = run.stack.len() - 1 - self.i;
        run.stack[ix] = v;
    }
}

/// APPEND ( |= ) statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GAppend {
    pub i: usize,
    pub exp: GExp,
}

impl GAppend {
    pub fn exec(&self, run: &mut Run) {
        let v = self.exp.eval(run);
        let ix = run.stack.len() - 1 - self.i;
        append(&mut run.stack[ix], &v);
    }
}

/// WHILE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GWhile<S: XString> {
    pub exp: GExp,
    pub block: GVec<GStatement<S>>,
}

impl<S: XString> GWhile<S> {
    pub fn exec(&self, run: &mut Run) {
        while self.exp.eval(run).bool() {
            execute_gblock(&self.block, run);
        }
    }
}

/// IF statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GIf<S: XString> {
    pub exp: GExp,
    pub block: GVec<GStatement<S>>,
    pub els: Option<GVec<GStatement<S>>>,
}

impl<S: XString> GIf<S> {
    pub fn exec(&self, run: &mut Run) {
        if self.exp.eval(run).bool() {
            execute_gblock(&self.block, run);
        } else if let Some(els) = &self.els {
            execute_gblock(els, run);
        }
    }
}

/// INSERT statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GInsert {
    pub table: Arc<STable>,
    pub cols: GVec<usize>,
    pub vals: GVec<GExp>,
}

impl GInsert {
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
            "GInsert exec table record count={} row={:?}",
            table.record_count(),
            row
        );
    }
}

/// UPDATE statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GUpdate {
    pub table: Arc<STable>,
    pub assigns: GVec<(usize, GExp)>, // col num, Exp
    pub wher: GExp,
}

impl GUpdate {
    pub fn exec(&self, run: &mut Run) {
        let t = run.ps.load_table(self.table.id, &self.table.dt);
        let ids = gids(&t, &self.wher, run);
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
pub struct GDelete {
    pub table: Arc<STable>,
    pub wher: GExp,
}

impl GDelete {
    pub fn exec(&self, run: &mut Run) {
        let t = run.ps.load_table(self.table.id, &self.table.dt);
        let ids = gids(&t, &self.wher, run);
        let mut table = t.borrow_mut();
        for id in &ids {
            table.remove(*id, run.ps);
        }
    }
}

pub type GOrderBy = Option<(GVec<GExp>, GVec<bool>)>;

/// SELECT statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GSelect {
    pub vals: GVec<GExp>,
    pub from: Option<Arc<STable>>,
    pub wher: Option<GExp>,
    pub order_by: GOrderBy,
}

impl GSelect {
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
        let temp = get_gtemp(f, &self.vals, &self.wher, &self.order_by, run);

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
pub struct GFor<S: XString> {
    pub vals: GVec<GExp>,
    pub from: Arc<STable>,
    pub wher: Option<GExp>,
    pub order_by: GOrderBy,
    pub block: GVec<GStatement<S>>,
}

impl<S: XString> GFor<S> {
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
                    execute_gblock(&self.block, run);
                    run.stack.truncate(len);
                }
            }
        }
    }
    pub fn exec_order_by(&self, run: &mut Run) {
        let temp = get_gtemp(&self.from, &self.vals, &self.wher, &self.order_by, run);

        let n = self.order_by.as_ref().unwrap().0.len();

        for row in &temp {
            let len = run.stack.len();
            for v in &row[n..] {
                run.stack.push(v.clone());
            }
            execute_gblock(&self.block, run);
            run.stack.truncate(len);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GStatement<S: XString> {
    /// Declare and initialise a local variable.
    Let(GLet<S>),
    /// Assign a local variable.
    Set(GSet),
    /// Append to a local string or binary variable.
    Append(GAppend),
    /// While loop.
    While(GWhile<S>),
    /// Conditional evalaution.
    If(GIf<S>),
    /// Insert into table.
    Insert(GInsert),
    /// Update table rows. Where condition is not optional, use "where true" to update all rows.
    Update(GUpdate),
    /// Delete rows from table. Where condition is not optional, use "where true" to delete all rows.
    Delete(GDelete),
    /// Output values.
    Select(GSelect),
    /// Loop through table, local variables are assigned to expressions evaluated from table rows.
    For(GFor<S>),
}

impl<S> GStatement<S>
where
    S: XString,
{
    /// Convert ....todo....
    pub fn from(stat: &Statement) -> GStatement<S> {
        match stat {
            Statement::Let(x) => GStatement::Let(GLet {
                varname: S::from_str(x.varname),
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

pub fn execute_fn<S>(f: &SFunc<S>, run: &mut Run)
where
    S: XString,
{
    // println!("execute_fn f={:?}", f);
    execute_gblock(&f.block, run);
}

fn execute_gblock<S>(slist: &[GStatement<S>], run: &mut Run)
where
    S: XString,
{
    let slen = run.stack.len(); // At end restore stack to this length.
    for s in slist {
        use GStatement::*;
        match s {
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
        };
    }
    run.stack.truncate(slen); // pop local variables from stack.
}

/// Get a list of ids for records from table that satisfy where condition.
fn gids(t: &RTable, wher: &GExp, run: &mut Run) -> LVec<i64> {
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

fn gvals(list: &[Exp]) -> GVec<GExp> {
    let mut result = GVec::with_capacity(list.len());
    for e in list {
        result.push(GExp::from(e));
    }
    result
}

pub fn gblock<S>(list: &[Statement]) -> GVec<GStatement<S>>
where
    S: XString,
{
    let mut block = GVec::with_capacity(list.len());
    for s in list {
        block.push(GStatement::from(s));
    }
    block
}

fn gorder_by(list: &OrderBy) -> GOrderBy {
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

fn get_gtemp(
    st: &STable,
    vals: &[GExp],
    wher: &Option<GExp>,
    order_by: &GOrderBy,
    run: &mut Run,
) -> LVec<LVec<Value>> {
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

pub type FStatement = GStatement<NoString>;
pub type FXStatement = GStatement<YesString>;

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
