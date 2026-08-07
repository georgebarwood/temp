use crate::*;
use serde::*;

/* Experiment...
   Idea is that stronger typed expression eval more efficiently as fewer internal Values to evaluate.


   Overview of whole process:

   During parsing, only constants (bool,int,string) are represented by Bool, Int, Str variants.
   Name resolution applies to Name and Binary variants ( and Builtin when that is done ).
   Name changes to BoolExp::Local, StrExp::Local or IntExp::Local variants.

   Next, if it is a stored function, it is converted from Local to Perm allocation,
   and any strings are converted from SrcPos to GString etc.

   Then, for executable version, it is encoded for execution (before being place in Dict).
   If it is not a stored function, it is simply encoded for temporary execution.
*/

/// Expression.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum Exp<A: Allocator + Debug + Default> {
    #[default]
    None,

    /// Boolean constant.
    Bool(BoolExp<A>),

    /// Integer constant.
    Int(IntExp<A>),

    /// String constant.
    Str(StrExp<A>),

    /// Unresolved Name, changes to Local or Col.
    Name(SrcPos),

    /// Local variable.
    Local(usize),

    /// Table column.
    Col(usize),

    /// Binary expression.
    Binary(Operator, BoxA<Exp<A>, A>, BoxA<Exp<A>, A>),

    /// Unresolved function call. Schema, fname, args.
    FnCallByName(SrcPos, SrcPos, VecA<Exp<A>, A>),

    /// Function call (resolved). Function id and args.
    FnCall(usize, VecA<Exp<A>, A>),

    /// Built-in call. Builtin operation and args.
    BuiltinCall(Builtin, VecA<Exp<A>, A>),

    /// Conditional expression. if b1 e1 if b2 e2 ... else e_def
    If(VecA<(Exp<A>, Exp<A>), A>, BoxA<Exp<A>, A>),

    /// Default expression.
    Default(DataType),
}

impl<A: Allocator + Debug + Default> Eval<Value> for Exp<A> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> Result<Value, E> {
        use Exp::*;
        Ok(match self {
            Bool(x) => Value::Bool(x.ev(run, rc)?),
            Int(x) => Value::Int(x.ev(run, rc)?),
            Str(x) => Value::String(LRc::new(x.ev(run, rc)?)),
            Local(x) => run.local(*x).clone(),
            Col(x) => rc.item(*x, run.ps),
            Binary(op, x, y) => {
                let x = x.ev(run, rc)?;
                let y = y.ev(run, rc)?;
                op.eval(&x, &y)
            }
            FnCall(f, args) => {
                let f = run.call_init(*f);
                let save = run.stack.len();
                for e in args {
                    let v = e.ev(run, rc)?;
                    run.stack.push(v);
                }
                execute_block_no_restore(&f.block, run)?;
                run.stack.truncate(save);
                run.stack.pop().unwrap() // Pop return value.
            }
            BuiltinCall(bi, args) => {
                for e in args {
                    let v = e.ev(run, rc)?;
                    run.stack.push(v);
                }
                bi.eval(run)
            }
            If(list, els) => {
                for (ce, e) in list {
                    if ce.ev(run, rc)?.bool() {
                        return e.ev(run, rc);
                    }
                }
                els.ev(run, rc)?
            }
            Default(dt) => dt.default_value(),
            _ => {
                // println!("exp={:?}", self);
                panic!()
            }
        })
    }
}

impl<A: Allocator + Debug + Default> Exp<A> {
    /// Convert from Local allocator.
    pub fn from(exp: &Exp<Local>, src: &[u8]) -> Self {
        use Exp::*;
        match exp {
            Bool(x) => Bool(BoolExp::from(x, src)),
            Int(x) => Int(IntExp::from(x, src)),
            Str(x) => Str(StrExp::from(x, src)),
            Local(x) => Local(*x),
            Col(x) => Col(*x),
            Binary(op, lhs, rhs) => {
                let lhs = BoxA::new(Self::from(lhs, src));
                let rhs = BoxA::new(Self::from(rhs, src));
                Binary(*op, lhs, rhs)
            }
            FnCall(fid, args) => {
                let args = gvals(args, src);
                FnCall(*fid, args)
            }
            BuiltinCall(bi, args) => {
                let args = gvals(args, src);
                BuiltinCall(*bi, args)
            }
            If(list, els) => {
                let mut x = VecA::new();
                for (ce, e) in list {
                    let ce = Self::from(ce, src);
                    let e = Self::from(e, src);
                    x.push((ce, e))
                }
                let els = BoxA::new(Self::from(els, src));
                If(x, els)
            }
            _ => todo!("Exp eval{:?}", exp),
        }
    }

    /// Checks whether an expresion has any column references.
    pub fn has_col(&self) -> bool {
        use Exp::*;
        match self {
            Col(_) => true,
            Binary(_, lhs, rhs) => lhs.has_col() || rhs.has_col(),
            FnCall(_, args) => {
                for e in args {
                    if e.has_col() {
                        return true;
                    }
                }
                false
            }
            BuiltinCall(_, args) => {
                for e in args {
                    if e.has_col() {
                        return true;
                    }
                }
                false
            }
            If(list, els) => {
                for (ce, e) in list {
                    if ce.has_col() || e.has_col() {
                        return true;
                    }
                }
                els.has_col()
            }
            _ => false,
        }
    }

    /// Walk the expression tree, noting any function calls.
    pub fn walk(&mut self, r: &mut URun)
    {
        use Exp::*;
        match self {
            Bool(BoolExp::Col(x)) => r.col(x),
            Int(IntExp::Col(x)) => r.col(x),
            Str(StrExp::Col(x)) => r.col(x),
            Binary(_, x, y) => {
               x.walk(r);
               y.walk(r);
            }
            Col(x) => {
               r.col(x)
            }
            FnCall(fid, args) => {
                for e in args {
                    e.walk(r);
                }
                r.fncall(fid);
            }
            BuiltinCall(_bi, args) => {
                for e in args {
                    e.walk(r);
                }
            }
            If(list, els) => {
                for (ce, e) in list {
                    ce.walk(r);
                    e.walk(r);
                }
                els.walk(r);
            }
            _ => {}
        }
    }   

    /// Encode for execution.
    /// Replace most Exp::Binary expressions, changing them to type specific Bool, Int or Str expressions.
    pub fn encode(&mut self) {
        // use std::ops::DerefMut;
        use Exp::*;
        match self {
            Binary(op, x, y) => {
                if *op == Operator::Concat {
                    return;
                }
                x.encode();
                y.encode();
                let re = match (op, &mut **x, &mut **y) {
                    (op, Bool(x), Bool(y)) => {
                        let x = BoxA::new(std::mem::take(x));
                        let y = BoxA::new(std::mem::take(y));
                        match op {
                            Operator::And => Bool(BoolExp::And(x, y)),
                            Operator::Or => Bool(BoolExp::Or(x, y)),
                            Operator::Equal => Bool(BoolExp::BoolEq(x, y)),
                            Operator::NotEqual => Bool(BoolExp::BoolNe(x, y)),
                            _ => todo!(),
                        }
                    }
                    (op, Int(x), Int(y)) => {
                        let x = BoxA::new(std::mem::take(x));
                        let y = BoxA::new(std::mem::take(y));
                        match op {
                            Operator::Plus => Int(IntExp::Add(x, y)),
                            Operator::Minus => Int(IntExp::Sub(x, y)),
                            Operator::Multiply => Int(IntExp::Mul(x, y)),
                            Operator::Divide => Int(IntExp::Div(x, y)),
                            Operator::Remainder => Int(IntExp::Rem(x, y)),
                            Operator::Equal => Bool(BoolExp::IntEq(x, y)),
                            Operator::NotEqual => Bool(BoolExp::IntNe(x, y)),
                            Operator::Less => Bool(BoolExp::IntLt(x, y)),
                            Operator::Greater => Bool(BoolExp::IntGt(x, y)),
                            Operator::LessEqual => Bool(BoolExp::IntLe(x, y)),
                            Operator::GreaterEqual => Bool(BoolExp::IntGe(x, y)),
                            _ => todo!(),
                        }
                    }
                    (op, Str(x), Str(y)) => {
                        let x = BoxA::new(std::mem::take(x));
                        let y = BoxA::new(std::mem::take(y));
                        match op {
                            Operator::Concat => Str(StrExp::Concat(x, y)),
                            Operator::Equal => Bool(BoolExp::StrEq(x, y)),
                            Operator::NotEqual => Bool(BoolExp::StrNe(x, y)),
                            Operator::Less => Bool(BoolExp::StrLt(x, y)),
                            Operator::Greater => Bool(BoolExp::StrGt(x, y)),
                            Operator::LessEqual => Bool(BoolExp::StrLe(x, y)),
                            Operator::GreaterEqual => Bool(BoolExp::StrGe(x, y)),
                            _ => todo!("Op={:?}", op),
                        }
                    }
                    _ => {
                        // println!("no encoding");
                        return;
                    }
                };
                *self = re;
            }
            FnCall(_fid, args) => {
                // Could have typed versions of FnCall.
                for e in args {
                    e.encode();
                }
            }
            BuiltinCall(_bi, args) => {
                // Could have typed versions of BuilinCall.
                for e in args {
                    e.encode();
                }
            }
            If(list, els) => {
                for (ce, e) in list {
                    ce.encode();
                    e.encode();
                }
                els.encode();
            }
            _ => {}
        }
    }

    /// New local variable, variant chosen based on datatype.
    pub fn local(x: usize, dt: &DataType) -> Self {
        match dt {
            DataType::Bool => Exp::Bool(BoolExp::Local(x)),
            DataType::Int => Exp::Int(IntExp::Local(x)),
            DataType::String(_) => Exp::Str(StrExp::Local(x)),
            _ => Exp::Local(x),
        }
    }

    /// New column, variant chosen based on datatype.
    pub fn col(x: usize, dt: &DataType) -> Self {
        match dt {
            DataType::Bool => Exp::Bool(BoolExp::Col(x)),
            DataType::Int => Exp::Int(IntExp::Col(x)),
            DataType::String(_) => Exp::Str(StrExp::Col(x)),
            _ => Exp::Col(x),
        }
    }

    /// Show expression.
    pub fn show(&self, sr: &mut SRun) -> Result<(), std::fmt::Error> {
        self.show_prec(sr, 0, false)
    }

    /// Show with specified precedence.
    fn show_prec(&self, sr: &mut SRun, pp: u8, right: bool) -> Result<(), std::fmt::Error> {
        use Exp::*;
        use std::fmt::Write;
        match self {
            Local(x) | Bool(BoolExp::Local(x)) | Int(IntExp::Local(x)) | Str(StrExp::Local(x)) => {
                sr.write_local_name(*x)
            }

            Col(x) | Bool(BoolExp::Col(x)) | Int(IntExp::Col(x)) | Str(StrExp::Col(x)) => {
                sr.write_col_name(*x)
            }

            // Constants.
            Bool(BoolExp::Bool(x)) => write!(&mut sr.output, "{}", x)?,
            Int(IntExp::Int(x)) => write!(&mut sr.output, "{}", x)?,
            Str(x) => x.show(sr)?, // For string constants,

            Binary(op, x, y) => {
                let p = op.precedence();
                if p < pp || p == pp && right {
                    sr.show("(");
                }
                x.show_prec(sr, p, false)?;
                write!(&mut sr.output, " {} ", op)?;
                y.show_prec(sr, p, true)?;
                if p < pp || p == pp && right {
                    sr.show(")");
                }
            }
            FnCall(f, args) => {
                sr.write_fn_name(*f);
                Self::show_args(args, sr, true)?;
            }
            BuiltinCall(bi, args) => {
                write!(&mut sr.output, "sys.{:?}", bi)?;
                Self::show_args(args, sr, false)?;
            }
            If(list, els) => {
                for (ce, e) in list {
                    sr.show("if ");
                    ce.show(sr)?;
                    sr.show(" ");
                    e.show(sr)?;
                    sr.show(" ");
                }
                sr.show("else ");
                els.show(sr)?;
            }
            Default(dt) => {
                write!(&mut sr.output, "default({})", dt)?;
            }
            _ => panic!(),
        }
        Ok(())
    }

    /// Show args.
    fn show_args(args: &[Exp<A>], sr: &mut SRun, ros: bool) -> Result<(), std::fmt::Error> {
        sr.output.push('(');
        let save = sr.aos;
        if ros {
            sr.aos += 1;
        }
        for (i, e) in args.iter().enumerate() {
            if i > 0 {
                sr.show(", ");
            }
            e.show(sr)?;
            sr.aos += 1;
        }
        sr.output.push(')');
        sr.aos = save;
        Ok(())
    }
} // end impl Exp

//////////////////////////////////////////////////////////////////////////////////////

/// Position of string in source.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SrcPos {
    pub start: usize,
    pub end: usize,
}

impl XString for SrcPos {
    fn sstr<'a>(&self, src: &'a [u8]) -> &'a str {
        tos(&src[self.start..self.end])
    }
    fn from_str(_s: &str) -> Self {
        panic!()
    }
}

/// No row context, for [`Exp::eval`].
struct NoRowContext;
impl RowContext for NoRowContext {
    fn item(&mut self, _i: usize, _ps: &mut PageSet) -> Value {
        panic!()
    }
}

/// Row context that is list of values, for [`Exp::eval_vals`].
struct ValsRowContext<'a> {
    vals: &'a [Value],
}

impl<'a> RowContext for ValsRowContext<'a> {
    fn item(&mut self, item: usize, _ps: &mut PageSet) -> Value {
        self.vals[item].clone()
    }
}

pub trait Eval<T> { // Not a good name
    /// Evaluate the expression with specified row context.
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> Result<T, E>;

    /// Evaluate the expression, no row context.
    fn eval(&self, run: &mut Run) -> Result<T, E> {
        self.ev(run, &mut NoRowContext)
    }

    /// Evaluate the expression using specified row values.
    fn eval_vals(&self, run: &mut Run, vals: &[Value]) -> Result<T, E> {
        let mut vc = ValsRowContext { vals };
        self.ev(run, &mut vc)
    }
}

//////////////////////////////////////////////////////////////////////////////////////

/// Bool Expression.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum BoolExp<A: Allocator + Debug + Default> {
    #[default]
    None,
    Bool(bool),
    Local(usize),
    Col(usize),
    And(BoxA<BoolExp<A>, A>, BoxA<BoolExp<A>, A>),
    Or(BoxA<BoolExp<A>, A>, BoxA<BoolExp<A>, A>),
    BoolEq(BoxA<BoolExp<A>, A>, BoxA<BoolExp<A>, A>),
    BoolNe(BoxA<BoolExp<A>, A>, BoxA<BoolExp<A>, A>),

    IntEq(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntNe(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntLt(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntGt(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntLe(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntGe(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),

    StrEq(BoxA<StrExp<A>, A>, BoxA<StrExp<A>, A>),
    StrNe(BoxA<StrExp<A>, A>, BoxA<StrExp<A>, A>),
    StrLt(BoxA<StrExp<A>, A>, BoxA<StrExp<A>, A>),
    StrGt(BoxA<StrExp<A>, A>, BoxA<StrExp<A>, A>),
    StrLe(BoxA<StrExp<A>, A>, BoxA<StrExp<A>, A>),
    StrGe(BoxA<StrExp<A>, A>, BoxA<StrExp<A>, A>),
    // String comparison is todo
}

impl<A: Allocator + Debug + Default> Eval<bool> for BoolExp<A> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> Result<bool, E> {
        use BoolExp::*;
        Ok(match self {
            None => panic!(),
            Bool(x) => *x,
            Local(x) => run.local(*x).bool(),
            Col(x) => rc.item(*x, run.ps).bool(),
            And(x, y) => x.ev(run, rc)? && y.ev(run, rc)?,
            Or(x, y) => x.ev(run, rc)? || y.ev(run, rc)?,
            BoolEq(x, y) => x.ev(run, rc)? == y.ev(run, rc)?,
            BoolNe(x, y) => x.ev(run, rc)? != y.ev(run, rc)?,

            IntEq(x, y) => x.ev(run, rc)? == y.ev(run, rc)?,
            IntNe(x, y) => x.ev(run, rc)? != y.ev(run, rc)?,
            IntLt(x, y) => x.ev(run, rc)? < y.ev(run, rc)?,
            IntGt(x, y) => x.ev(run, rc)? > y.ev(run, rc)?,
            IntLe(x, y) => x.ev(run, rc)? <= y.ev(run, rc)?,
            IntGe(x, y) => x.ev(run, rc)? >= y.ev(run, rc)?,

            StrEq(x, y) => x.ev(run, rc)? == y.ev(run, rc)?,
            StrNe(x, y) => x.ev(run, rc)? != y.ev(run, rc)?,
            StrLt(x, y) => x.ev(run, rc)? < y.ev(run, rc)?,
            StrGt(x, y) => x.ev(run, rc)? > y.ev(run, rc)?,
            StrLe(x, y) => x.ev(run, rc)? <= y.ev(run, rc)?,
            StrGe(x, y) => x.ev(run, rc)? >= y.ev(run, rc)?,
        })
    }
}

impl<A: Allocator + Debug + Default> BoolExp<A> {
    pub fn from(exp: &BoolExp<Local>, _src: &[u8]) -> Self {
        match exp {
            BoolExp::Bool(x) => BoolExp::Bool(*x),
            BoolExp::Local(x) => BoolExp::Local(*x),
            BoolExp::Col(x) => BoolExp::Col(*x),
            _ => panic!(),
        }
    } 
}

/// Integer Expression.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum IntExp<A: Allocator + Debug + Default> {
    #[default]
    None,
    Int(i64),
    Local(usize),
    Col(usize),
    Add(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    Sub(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    Mul(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    Div(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    Rem(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
}

impl<A: Allocator + Debug + Default> Eval<i64> for IntExp<A> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> Result<i64, E> {
        use IntExp::*;
        Ok(match self {
            None => panic!(),
            Int(x) => *x,
            Local(x) => run.local(*x).int(),
            Col(x) => rc.item(*x, run.ps).int(),
            Add(lhs, rhs) => lhs.ev(run, rc)? + rhs.ev(run, rc)?,
            Sub(lhs, rhs) => lhs.ev(run, rc)? - rhs.ev(run, rc)?,
            Mul(lhs, rhs) => lhs.ev(run, rc)? * rhs.ev(run, rc)?,
            Div(lhs, rhs) => {
                let lhs = lhs.ev(run, rc)?;
                let rhs = rhs.ev(run, rc)?;
                if rhs == 0 {
                    return Err(E::new("Divide by zero"));
                }
                lhs / rhs
            }
            Rem(lhs, rhs) => {
                let lhs = lhs.ev(run, rc)?;
                let rhs = rhs.ev(run, rc)?;
                if rhs == 0 {
                    return Err(E::new("Divide by zero"));
                }
                lhs % rhs
            }
        })
    }
}

impl<A: Allocator + Debug + Default> IntExp<A> {
    /// Convert from Local allocator.
    pub fn from(exp: &IntExp<Local>, _src: &[u8]) -> Self {
        match exp {
            IntExp::Int(x) => IntExp::Int(*x),
            IntExp::Local(x) => IntExp::Local(*x),
            IntExp::Col(x) => IntExp::Col(*x),
            _ => panic!(),
        }
    }
}

/// String Expression.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum StrExp<A: Allocator + Debug + Default> {
    #[default]
    None,
    Local(usize),
    Col(usize),
    Str(GString),
    StrPos(SrcPos),
    Concat(BoxA<StrExp<A>, A>, BoxA<StrExp<A>, A>),
}

impl<A: Allocator + Debug + Default> Eval<LString> for StrExp<A> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> Result<LString, E> {
        use StrExp::*;
        Ok(match self {
            None => panic!(),
            Local(x) => LString::from(run.local(*x).string().as_str()),
            Col(x) => LString::from(rc.item(*x, run.ps).string().as_str()),
            Str(x) => LString::from(x.as_str()),
            StrPos(x) => LString::from(x.sstr(run.source.as_bytes())),
            Concat(lhs, rhs) => {
                let mut lhs = lhs.ev(run, rc)?;
                let rhs = rhs.ev(run, rc)?;
                lhs.push_str(&rhs);
                lhs
            }
        })
    }
}

impl<A: Allocator + Debug + Default> StrExp<A> {
    /// Convert from Local allocator.
    pub fn from(exp: &StrExp<Local>, src: &[u8]) -> Self {
        match exp {
            StrExp::Str(x) => StrExp::Str(GString::from(x.as_str())),
            StrExp::Local(x) => StrExp::Local(*x),
            StrExp::Col(x) => StrExp::Col(*x),
            StrExp::StrPos(x) => StrExp::Str(GString::from(x.sstr(src))),
            _ => todo!(),
        }
    }

    pub fn show(&self, sr: &mut SRun) -> Result<(), std::fmt::Error> {
        match self {
            StrExp::Str(x) => {
                str_literal(x.as_str(), &mut sr.output);
            }
            _ => todo!(),
        }
        Ok(())
    }
}
