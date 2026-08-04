use crate::*;
use serde::*;

/// Statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Statement<A: Allocator + Debug + Default, S: XString> {
    /// Null statement ( nothing to do on pass 2 )
    Null,
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
    /// Optimised For for case where Id = `<exp>`
    UpdateIdEq(UpdateIdEq<A>),
    /// Delete rows from table. Where condition is not optional, use "where true" to delete all rows.
    Delete(Delete<A>),
    /// Optimised For for case where Id = `<exp>`
    DeleteIdEq(DeleteIdEq<A>),
    /// Output values.
    Select(Select<A>),
    /// Optimised For for case where Id = `<exp>`
    SelectIdEq(SelectIdEq<A>),
    /// Loop through table, local variables are assigned to expressions evaluated from table rows.
    For(For<A, S>),
    /// Optimised For for case where Id = `<exp>`
    ForIdEq(ForIdEq<A, S>),
    /// schema.
    CreateSchema(CreateSchema),
    /// table
    CreateTable(CreateTable),

    /// alter table add column.
    AddColumn(AddColumn),

    /// Create Function.
    CreateFn(CreateFn<A>),

    /// Rename Schema.
    RenameSchema(RenameSchema),
    /// Rename Table.
    RenameTable(RenameTable),
    /// Rename Function.
    RenameFn(RenameFn),
    /// Rename Column.
    RenameColumn(RenameColumn),

    /// drop schema.
    DropSchema(DropSchema),
    /// drop table.
    DropTable(DropTable),
    /// alter table drop column.
    DropColumn(DropColumn),
    /// drop function.
    DropFn(DropFn),

    /// alter table add index.
    AddIndex(AddIndex),
}

use std::fmt::Write;

impl<A, S> Statement<A, S>
where
    A: Allocator + Debug + Default,
    S: XString,
{
    pub fn show<'a>(&'a self, sr: &mut SRun<'a>) -> Result<(), std::fmt::Error> {
        use Statement::*;
        match self {
            Let(x) => {
                write!(&mut sr.output, "let {} = ", x.varname.str())?;
                x.exp.show(sr)?;
                sr.names.push(x.varname.str());
            }
            Set(x) => {
                sr.show("set ");
                sr.write_local_name(x.i);

                sr.show(" = ");
                x.exp.show(sr)?;
            }
            Append(x) => {
                sr.show("set ");
                sr.write_local_name(x.i);

                sr.show(" |= ");
                x.exp.show(sr)?;
            }
            While(x) => {
                sr.show("while ");
                x.exp.show(sr)?;
                show_block(sr, &x.block)?;
            }
            If(x) => {
                sr.show("if ");
                x.exp.show(sr)?;
                show_block(sr, &x.block)?;
                if let Some(b) = &x.els {
                    sr.show(" else ");
                    show_block(sr, b)?;
                }
            }
            Insert(x) => {
                sr.show("insert into ");
                sr.set_table(x.table);
                sr.write_table_name();
                sr.show("(");
                for (i, c) in x.cols.iter().enumerate() {
                    if i != 0 {
                        sr.show(", ");
                    }
                    sr.write_col_name(*c);
                }
                sr.show(") values (");
                sr.table = None; // Optional
                for (i, e) in x.vals.iter().enumerate() {
                    if i != 0 {
                        sr.show(", ");
                    }
                    e.show(sr)?;
                }
                sr.show(")");
            }
            Update(x) => {
                sr.show("update ");
                sr.set_table(x.table);
                sr.write_table_name();
                sr.show(" set ");
                for (i, (c, e)) in x.assigns.iter().enumerate() {
                    if i != 0 {
                        sr.show(", ");
                    }
                    sr.write_col_name(*c);
                    sr.show(" = ");
                    e.show(sr)?;
                }
                sr.show(" where ");
                x.wher.show(sr)?;
            }
            Delete(x) => {
                sr.show("delete from ");
                sr.set_table(x.table);
                sr.write_table_name();
                sr.show(" where ");
                x.wher.show(sr)?;
            }
            Select(x) => {
                sr.show("select ");
                if let Some(from) = x.from {
                    sr.set_table(from);
                }
                for (i, e) in x.vals.iter().enumerate() {
                    if i != 0 {
                        if sr.col() > 50 {
                            sr.newln();
                        }
                        sr.show(", ");
                    }
                    e.show(sr)?;
                }
                if x.from.is_some() {
                    sr.newln();
                    sr.show("from ");
                    sr.write_table_name();
                    if let Some(w) = &x.wher {
                        sr.show(" where ");
                        w.show(sr)?;
                    }
                    Self::show_order_by(&x.order_by, sr)?;
                }
            }
            For(x) => {
                sr.show("for ");
                sr.set_table(x.from);
                for (i, (x, val)) in x.assigns.iter().enumerate() {
                    if i != 0 {
                        sr.show(", ");
                    }
                    sr.write_local_name(*x);
                    sr.show(" = ");
                    val.show(sr)?;
                }
                sr.show(" from ");
                sr.write_table_name();

                if let Some(w) = &x.wher {
                    sr.show(" where ");
                    w.show(sr)?;
                }
                Self::show_order_by(&x.order_by, sr)?;

                show_block(sr, &x.block)?;
            }
            _ => todo!(),
        }
        Ok(())
    }

    fn show_order_by(ob: &OrderBy<A>, sr: &mut SRun) -> Result<(), std::fmt::Error> {
        if let Some((list, desc)) = ob {
            sr.show(" order by ");
            for (i, e) in list.iter().enumerate() {
                if i != 0 {
                    sr.show(", ");
                }
                e.show(sr)?;
                if desc[i] {
                    sr.show(" desc ");
                }
            }
        }
        Ok(())
    }

    fn from(stat: &LStatement, src: &[u8]) -> Self {
        match stat {
            Statement::Let(x) => Statement::Let(Let {
                varname: S::from_str(x.varname.sstr(src)),
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
                let table = x.table;
                let cols = VecA::from(&*x.cols);
                let vals = gvals(&x.vals, src);
                Statement::Insert(Insert { table, cols, vals })
            }
            Statement::Select(x) => {
                let vals = gvals(&x.vals, src);
                let from = x.from;
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
                let assigns = gassigns(&x.assigns, src);
                let from = x.from;
                let wher = x.wher.as_ref().map(|wher| Exp::from(wher, src));
                let order_by = gorder_by(&x.order_by, src);
                let block = gblock(&x.block, src);
                Statement::For(For {
                    assigns,
                    from,
                    wher,
                    order_by,
                    block,
                })
            }
            Statement::Update(x) => {
                let table = x.table;
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
                let table = x.table;
                let wher = Exp::from(&x.wher, src);
                Statement::Delete(Delete { table, wher })
            }
            _ => panic!(),
        }
    }
}

/// let statement - declare and initialise a local variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Let<A: Allocator + Debug + Default, S: XString> {
    pub varname: S,
    pub exp: Exp<A>,
}

impl<A: Allocator + Debug + Default, S: XString> Let<A, S> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        let v = self.exp.eval(run)?;
        run.stack.push(v);
        Ok(())
    }
}

/// set statement - assign a local variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Set<A: Allocator + Debug + Default> {
    pub i: usize,
    pub exp: Exp<A>,
}

impl<A: Allocator + Debug + Default> Set<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        let v = self.exp.eval(run)?;
        *run.local(self.i) = v;
        Ok(())
    }
}

/// append ( |= ) statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Append<A: Allocator + Debug + Default> {
    pub i: usize,
    pub exp: Exp<A>,
}

impl<A: Allocator + Debug + Default> Append<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        let v = self.exp.eval(run)?;
        append(run.local(self.i), &v);
        Ok(())
    }
}

/// while statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct While<A: Allocator + Debug + Default, S: XString> {
    pub exp: Exp<A>,
    pub block: VecA<Statement<A, S>, A>,
}

impl<A: Allocator + Debug + Default, S: XString> While<A, S> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        while self.exp.eval(run)?.bool() {
            execute_block(&self.block, run)?;
        }
        Ok(())
    }
}

/// if statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct If<A: Allocator + Debug + Default, S: XString> {
    pub exp: Exp<A>,
    pub block: VecA<Statement<A, S>, A>,
    pub els: Option<VecA<Statement<A, S>, A>>,
}

impl<A: Allocator + Debug + Default, S: XString> If<A, S> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        if self.exp.eval(run)?.bool() {
            execute_block(&self.block, run)?;
        } else if let Some(els) = &self.els {
            execute_block(els, run)?;
        }
        Ok(())
    }
}

/// insert statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insert<A: Allocator + Debug + Default> {
    pub table: usize,
    pub cols: VecA<usize, A>,
    pub vals: VecA<Exp<A>, A>,
}

impl<A: Allocator + Debug + Default> Insert<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        run.check_write()?;

        // First evaluate the expressions.
        let mut ee = LVec::with_capacity(self.vals.len());
        for e in &self.vals {
            ee.push(e.eval(run)?);
        }
        let t = run.load_table(self.table);
        let mut table = t.try_borrow_mut()?;

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
        Ok(())
    }
}

/// update statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update<A: Allocator + Debug + Default> {
    pub table: usize,
    pub assigns: VecA<(usize, Exp<A>), A>, // col num, Exp
    pub wher: Exp<A>,
}

impl<A: Allocator + Debug + Default> Update<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        run.check_write()?;
        let t = run.load_table(self.table);
        let ids = ids(&t, &self.wher, run)?;
        let mut table = t.try_borrow_mut()?;
        for id in &ids {
            let mut row = table.fetch(*id, run.ps).unwrap();
            let mut vals = LVec::with_capacity(self.assigns.len());
            {
                for (_col, e) in &self.assigns {
                    let v = e.eval_vals(run, row.list())?;
                    vals.push(v);
                }
            }
            let mrow = LRc::make_mut(row.list_mut());
            for (col, _e) in self.assigns.iter().rev() {
                mrow[*col] = vals.pop().unwrap();
            }
            table.update(*id, &row, run.ps);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIdEq<A: Allocator + Debug + Default> {
    pub table: usize,
    pub assigns: VecA<(usize, Exp<A>), A>, // col num, Exp
    pub exp: IntExp<A>,
}

impl<A: Allocator + Debug + Default> UpdateIdEq<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        run.check_write()?;
        let id = self.exp.eval(run)?;
        let t = run.load_table(self.table);
        let mut table = t.try_borrow_mut()?;

        let mut row = table.fetch(id, run.ps).unwrap();
        let mut vals = LVec::with_capacity(self.assigns.len());
        {
            for (_col, e) in &self.assigns {
                let v = e.eval_vals(run, row.list())?;
                vals.push(v);
            }
        }
        let mrow = LRc::make_mut(row.list_mut());
        for (col, _e) in self.assigns.iter().rev() {
            mrow[*col] = vals.pop().unwrap();
        }
        table.update(id, &row, run.ps);
        Ok(())
    }
}

/// delete statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delete<A: Allocator + Debug + Default> {
    pub table: usize,
    pub wher: Exp<A>,
}

impl<A: Allocator + Debug + Default> Delete<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        run.check_write()?;
        let t = run.load_table(self.table);
        let ids = ids(&t, &self.wher, run)?;
        let mut table = t.try_borrow_mut()?;
        for id in &ids {
            table.remove(*id, run.ps);
        }
        Ok(())
    }
}

/// delete statement ( Id = )
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteIdEq<A: Allocator + Debug + Default> {
    pub table: usize,
    pub exp: IntExp<A>,
}

impl<A: Allocator + Debug + Default> DeleteIdEq<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        run.check_write()?;
        let id = self.exp.eval(run)?;
        let t = run.load_table(self.table);
        let mut table = t.try_borrow_mut()?;
        table.remove(id, run.ps);
        Ok(())
    }
}

/// order by clause.
pub type OrderBy<A> = Option<(VecA<Exp<A>, A>, VecA<bool, A>)>;

/// select statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Select<A: Allocator + Debug + Default> {
    pub vals: VecA<Exp<A>, A>,
    pub from: Option<usize>,
    pub wher: Option<Exp<A>>,
    pub order_by: OrderBy<A>,
}

impl<A: Allocator + Debug + Default> Select<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        if self.order_by.is_some() {
            self.exec_order_by(run)?;
        } else if let Some(f) = &self.from {
            let t = run.load_table(*f);
            let table = t.try_borrow()?;
            let mut iter = table.iter(run.ps);
            while let Some(b) = iter.next_ref(run.ps) {
                // print!("got a row :");
                let mut lr = table.lazy_row(b);
                let ok = if let Some(wher) = &self.wher {
                    wher.ev(run, &mut lr)?.bool()
                } else {
                    true
                };
                if ok {
                    for e in &self.vals {
                        let v = e.ev(run, &mut lr)?;
                        run.output(&v);
                    }
                }
            }
        } else {
            // SELECT with no FROM
            for e in &self.vals {
                let v = e.eval(run)?;
                run.output(&v);
            }
        }
        Ok(())
    }
    pub fn exec_order_by(&self, run: &mut Run) -> Result<(), E> {
        let f = self.from.unwrap();
        let temp = get_temp(f, &self.vals, &self.wher, &self.order_by, run)?;

        let n = self.order_by.as_ref().unwrap().0.len();
        for row in &temp {
            for v in &row[n..] {
                run.output(v);
            }
        }
        Ok(())
    }
}

/// select statement ( Id = `<exp>` case )
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectIdEq<A: Allocator + Debug + Default> {
    pub vals: VecA<Exp<A>, A>,
    pub from: Option<usize>,
    pub exp: IntExp<A>,
}

impl<A: Allocator + Debug + Default> SelectIdEq<A> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        let t = run.load_table(self.from.unwrap());
        let table = t.try_borrow()?;
        let id = self.exp.eval(run)?;
        if let Some(mut lr) = table.lazy_fetch(id, run.ps) {
            for e in &self.vals {
                let v = e.ev(run, &mut lr)?;
                run.output(&v);
            }
        }
        Ok(())
    }
}

/// for .. from .. statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct For<A: Allocator + Debug + Default, S: XString> {
    pub assigns: VecA<(usize, Exp<A>), A>,
    pub from: usize,
    pub wher: Option<Exp<A>>,
    pub order_by: OrderBy<A>,
    pub block: VecA<Statement<A, S>, A>,
}

impl<A: Allocator + Debug + Default, S: XString> For<A, S> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        if self.order_by.is_some() {
            self.exec_order_by(run)
        } else {
            let t = run.load_table(self.from);
            let table = t.try_borrow()?;
            let mut iter = table.iter(run.ps);
            while let Some(b) = iter.next_ref(run.ps) {
                let mut lr = table.lazy_row(b);

                let ok = if let Some(wher) = &self.wher {
                    let v = wher.ev(run, &mut lr)?;
                    v.bool()
                } else {
                    true
                };

                if ok {
                    for (i, e) in &self.assigns {
                        let v = e.ev(run, &mut lr)?;
                        *run.local(*i) = v;
                    }
                    execute_block(&self.block, run)?;
                }
            }
            Ok(())
        }
    }
    pub fn exec_order_by(&self, run: &mut Run) -> Result<(), E> {
        let temp = get_for_temp(self.from, &self.assigns, &self.wher, &self.order_by, run)?;

        let n = self.order_by.as_ref().unwrap().0.len();

        for row in &temp {
            for (c, v) in row[n..].iter().enumerate() {
                let i = self.assigns[c].0;
                *run.local(i) = v.clone(); // Maybe could avoid clone, but it is cheap.
            }
            execute_block(&self.block, run)?;
        }
        Ok(())
    }
}

/// for .. from .. statement where Id = `<exp>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForIdEq<A: Allocator + Debug + Default, S: XString> {
    pub assigns: VecA<(usize, Exp<A>), A>,
    pub from: usize,
    pub exp: IntExp<A>,
    pub block: VecA<Statement<A, S>, A>,
}

impl<A: Allocator + Debug + Default, S: XString> ForIdEq<A, S> {
    fn exec(&self, run: &mut Run) -> Result<(), E> {
        // println!("ForIdEq::exec!!");
        let t = run.load_table(self.from);
        let table = t.try_borrow()?;
        let id = self.exp.eval(run)?;
        if let Some(mut lr) = table.lazy_fetch(id, run.ps) {
            for (i, e) in &self.assigns {
                let v = e.ev(run, &mut lr)?;
                *run.local(*i) = v;
            }
            execute_block(&self.block, run)?;
        }
        Ok(())
    }
}

/// Create schema statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSchema {
    pub sname: SrcPos,
}

/// Rename schema statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameSchema {
    pub schema_id: i64,
    pub new_name: SrcPos,
}

/// Create table statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTable {
    pub schema_id: i64,
    pub tname: SrcPos,
    pub col_defs: DataType,
}

/// Rename table statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameTable {
    pub old_schema_id: i64,
    pub old_nid: i64,
    pub new_schema_id: i64,
    pub new_tname: SrcPos,
}

/// create / alter fn statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFn<A: Allocator + Debug + Default> {
    pub schema_id: i64,
    pub fname: SrcPos,
    pub ret: DataType,
    pub parms: VecA<(SrcPos, DataType), A>,
    pub block: VecA<Statement<A, SrcPos>, A>,
    pub alter: bool,
}

/// rename fn statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameFn {
    pub old_schema_id: i64,
    pub old_nid: i64,
    pub new_schema_id: i64,
    pub new_fname: SrcPos,
}

/// alter table add column statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddColumn {
    pub table_id: usize,
    pub col_name: SrcPos,
    pub col_dt: DataType,
}

/// alter table add index statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddIndex {
    pub table_id: usize,
    pub col_num: usize,
}

/// drop column statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropColumn {
    pub table_id: usize,
    pub col_num: usize,
}

/// rename column statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameColumn {
    pub table_id: usize,
    pub col_num: usize,
    pub new_name: SrcPos,
}

/// drop schema statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropSchema {
    pub schema_id: i64,
}

/// drop table statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropTable {
    pub schema_id: i64,
    pub name_id: i64,
    pub table: usize,
}

/// drop function statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropFn {
    pub function_id: usize,
}

/// Execute list of statements, restoring stack to original len.
pub fn execute_block<A, S>(slist: &[Statement<A, S>], run: &mut Run) -> Result<(), E>
where
    A: Allocator + Debug + Default,
    S: XString,
{
    let slen = run.stack.len(); // At end restore stack to this length.
    execute_block_no_restore(slist, run)?;
    run.stack.truncate(slen); // pop local variables from stack.
    Ok(())
}

/// Execute list of statements ( caller must restore stack ).
pub fn execute_block_no_restore<A, S>(slist: &[Statement<A, S>], run: &mut Run) -> Result<(), E>
where
    A: Allocator + Debug + Default,
    S: XString,
{
    for s in slist {
        use Statement::*;
        match s {
            Let(x) => x.exec(run),
            Set(x) => x.exec(run),
            Append(x) => x.exec(run),
            While(x) => x.exec(run),
            If(x) => x.exec(run),
            Insert(x) => x.exec(run),
            Update(x) => x.exec(run),
            UpdateIdEq(x) => x.exec(run),
            Delete(x) => x.exec(run),
            DeleteIdEq(x) => x.exec(run),
            Select(x) => x.exec(run),
            SelectIdEq(x) => x.exec(run),
            For(x) => x.exec(run),
            ForIdEq(x) => x.exec(run),
            CreateSchema(_) | CreateTable(_) | RenameTable(_) | CreateFn(_) | RenameFn(_)
            | DropFn(_) | DropTable(_) | AddColumn(_) | DropColumn(_) | DropSchema(_)
            | RenameSchema(_) | RenameColumn(_) | AddIndex(_) | Null => panic!(),
        }?;
    }
    Ok(())
}

/// Get a list of ids for records from table that satisfy where condition.
fn ids<A>(t: &RTable, wher: &Exp<A>, run: &mut Run) -> Result<LVec<i64>, E>
where
    A: Allocator + Debug + Default,
{
    let mut result = LVec::new();
    let table = t.try_borrow()?;
    let mut iter = table.iter(run.ps);
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        if wher.ev(run, &mut lr)?.bool() {
            let id = lr.item(0, run.ps).int();
            result.push(id);
        }
    }
    Ok(result)
}

/// Convert list of local expressions to new allocator.
pub fn gvals<A>(list: &[LExp], src: &[u8]) -> VecA<Exp<A>, A>
where
    A: Allocator + Debug + Default,
{
    let mut result = VecA::with_capacity(list.len());
    for e in list {
        result.push(Exp::from(e, src));
    }
    result
}

/// Convert list of assigns to new allocatpr.
pub fn gassigns<A>(list: &[(usize, LExp)], src: &[u8]) -> VecA<(usize, Exp<A>), A>
where
    A: Allocator + Debug + Default,
{
    let mut result = VecA::with_capacity(list.len());
    for (x, e) in list {
        result.push((*x, Exp::from(e, src)));
    }
    result
}

/// Convert list of local statements to new allocator.
pub fn gblock<A, S>(list: &[LStatement], src: &[u8]) -> VecA<Statement<A, S>, A>
where
    A: Allocator + Debug + Default,
    S: XString,
{
    let mut block = VecA::with_capacity(list.len());
    for s in list {
        block.push(Statement::from(s, src));
    }
    block
}

/// Convert local Order By to new allocator.
fn gorder_by<A: Allocator + Debug + Default>(list: &LOrderBy, src: &[u8]) -> OrderBy<A> {
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

/// Get filtered, sorted temporary table.
fn get_for_temp<A: Allocator + Debug + Default, S>(
    table_id: usize,
    lets: &[(S, Exp<A>)],
    wher: &Option<Exp<A>>,
    order_by: &OrderBy<A>,
    run: &mut Run,
) -> Result<LVec<LVec<Value>>, E> {
    let (ob, desc) = order_by.as_ref().unwrap();
    let table = run.load_table(table_id);
    let table = table.try_borrow()?;
    let mut iter = table.iter(run.ps);

    let mut temp = LVec::new();
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        let ok = if let Some(wher) = &wher {
            wher.ev(run, &mut lr)?.bool()
        } else {
            true
        };
        if ok {
            let mut row = LVec::with_capacity(ob.len() + lets.len());
            for e in ob {
                let v = e.ev(run, &mut lr)?;
                row.push(v);
            }
            for (_, e) in lets {
                let v = e.ev(run, &mut lr)?;
                row.push(v);
            }
            temp.push(row);
        }
    }
    temp.sort_by(|a, b| row_compare(a, b, desc));
    Ok(temp)
}

/// Get filtered, sorted temporary table.
fn get_temp<A>(
    table_id: usize,
    vals: &[Exp<A>],
    wher: &Option<Exp<A>>,
    order_by: &OrderBy<A>,
    run: &mut Run,
) -> Result<LVec<LVec<Value>>, E>
where
    A: Allocator + Debug + Default,
{
    let (ob, desc) = order_by.as_ref().unwrap();
    let table = run.load_table(table_id);
    let table = table.try_borrow()?;
    let mut iter = table.iter(run.ps);

    let mut temp = LVec::new();
    while let Some(b) = iter.next_ref(run.ps) {
        let mut lr = table.lazy_row(b);
        let ok = if let Some(wher) = &wher {
            wher.ev(run, &mut lr)?.bool()
        } else {
            true
        };
        if ok {
            let mut row = LVec::with_capacity(ob.len() + vals.len());
            for e in ob {
                let v = e.ev(run, &mut lr)?;
                row.push(v);
            }
            for e in vals {
                let v = e.ev(run, &mut lr)?;
                row.push(v);
            }
            temp.push(row);
        }
    }
    temp.sort_by(|a, b| row_compare(a, b, desc));
    Ok(temp)
}
