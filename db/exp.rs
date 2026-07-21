use crate::*;
use serde::*;

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

/**************************************************

/// Expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Exp<A: Allocator + Default> {
    /// Bool constant
    Bool(bool),
    /// Integer constant
    Int(i64),
    /// String literal
    String(StringA<A>),
    /// String literal ( position in source )
    SrcString(SrcPos),
    /// Unresolved name
    Name(SrcPos),
    /// Local variable.
    Local(usize),
    /// Column number
    Col(usize),
    /// Binary expression.
    Binary(Operator, BoxA<Exp<A>, A>, BoxA<Exp<A>, A>),
    /// Function call (unresolved). Schema, fname, args.
    FnCallByName(SrcPos, SrcPos, VecA<Exp<A>, A>),
    /// Function call (resolved). Function id and args.
    FnCall(usize, VecA<Exp<A>, A>),
    /// Built-in call. Build-in operation and args.
    CallBuiltin(Builtin, VecA<Exp<A>, A>),
}

impl<A: Allocator + Default> Exp<A> {
    /// Evaluate the expression using specified Run and RowContext.
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> Value {
        use Exp::*;
        match self {
            Bool(x) => Value::Bool(*x),
            Int(x) => Value::Int(*x),
            SrcString(x) => {
                let s = x.sstr(run.source);
                Value::String(LRc::new(LString::from(s)))
            }
            String(x) => Value::String(LRc::new(LString::from(x.as_ref()))),
            Local(x) => {
                let ix = run.stack.len() - (x + 1);
                run.stack[ix].clone()
            }
            Col(x) => rc.item(*x, run.ps),
            Binary(op, x, y) => {
                let x = x.ev(run, rc);
                let y = y.ev(run, rc);
                op.eval(&x, &y)
            }
            FnCall(f, args) => {
                let f = run.call_init(*f);
                let save = run.stack.len();
                for e in args {
                    let v = e.ev(run, rc);
                    run.stack.push(v);
                }
                execute_block(&f.block, run);
                run.stack.truncate(save);
                run.stack.pop().unwrap() // Pop return value.
            }
            CallBuiltin(bi, args) => {
                for e in args {
                    let v = e.ev(run, rc);
                    run.stack.push(v);
                }
                bi.eval(run)
            }
            Name(_) | FnCallByName(_, _, _) => panic!(),
        }
    }

    /// Evaluate the expression, no row context.
    pub fn eval(&self, run: &mut Run) -> Value {
        self.ev(run, &mut NoRowContext)
    }

    /// Evaluate the expression using specified row values.
    pub fn eval_vals(&self, run: &mut Run, vals: &[Value]) -> Value {
        let mut vc = ValsRowContext { vals };
        self.ev(run, &mut vc)
    }

    /// Convert from LExp
    pub fn from(exp: &LExp, src: &[u8]) -> Self {
        use Exp::*;
        match exp {
            Bool(x) => Bool(*x),
            Int(x) => Int(*x),
            SrcString(x) => String(StringA::from(x.sstr(src))),
            String(x) => String(StringA::from(x.as_str())),
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
            CallBuiltin(bi, args) => {
                let args = gvals(args, src);
                CallBuiltin(*bi, args)
            }
            Name(_) | FnCallByName(_, _, _) => panic!(), // Names have been resolved by this point.
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
            Bool(x) => write!(&mut sr.output, "{}", x)?,
            Int(x) => write!(&mut sr.output, "{}", x)?,
            String(x) => {
                sr.output.push_str("'");
                sr.output.push_str(x);
                sr.output.push_str("'");
            }
            Local(x) => {
                sr.write_name(*x);
            }
            Col(x) => {
                sr.write_col_name(*x);
            }
            Binary(op, x, y) => {
                let p = op.precedence();
                if p < pp || p == pp && right {
                    sr.output.push_str("(");
                }
                x.show_prec(sr, p, false)?;
                write!(&mut sr.output, " {} ", op)?;
                y.show_prec(sr, p, true)?;
                if p < pp || p == pp && right {
                    sr.output.push_str(")");
                }
            }
            FnCall(f, args) => {
                sr.write_fn_name(*f);
                Self::show_args(args, sr)?;
            }
            CallBuiltin(bi, args) => {
                write!(&mut sr.output, "sys.{:?}", bi)?;
                Self::show_args(args, sr)?;
            }
            _ => panic!(),
        }
        Ok(())
    }

    /// Show args.
    fn show_args(args: &[Exp<A>], sr: &mut SRun) -> Result<(), std::fmt::Error> {
        sr.output.push('(');
        let save = sr.aos;
        sr.aos += 1;
        for (i, e) in args.iter().enumerate() {
            if i > 0 {
                sr.output.push_str(", ");
            }
            e.show(sr)?;
            sr.aos += 1;
        }
        sr.output.push(')');
        sr.aos = save;
        Ok(())
    }
}

*/

//////////////////////////////

pub trait Eval<T> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> T;

    /// Evaluate the expression, no row context.
    fn eval(&self, run: &mut Run) -> T {
        self.ev(run, &mut NoRowContext)
    }

    /// Evaluate the expression using specified row values.
    fn eval_vals(&self, run: &mut Run, vals: &[Value]) -> T {
        let mut vc = ValsRowContext { vals };
        self.ev(run, &mut vc)
    }
}

/// Bool Expression.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum BoolExp<A: Allocator + Default> {
    #[default]
    None,
    Bool(bool),
    Local(usize),
    Col(usize),
    And(BoxA<BoolExp<A>, A>, BoxA<BoolExp<A>, A>),
    Or(BoxA<BoolExp<A>, A>, BoxA<BoolExp<A>, A>),
    IntEq(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntNe(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntLt(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntGt(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntLe(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    IntGe(BoxA<IntExp<A>, A>, BoxA<IntExp<A>, A>),
    // Many more todo!
}

impl<A: Allocator + Default> Eval<bool> for BoolExp<A> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> bool {
        use BoolExp::*;
        match self {
            None => panic!(),
            Bool(x) => *x,
            And(x, y) => x.ev(run, rc) && y.ev(run, rc),
            Or(x, y) => x.ev(run, rc) || y.ev(run, rc),
            IntEq(x, y) => x.ev(run, rc) == y.ev(run, rc),
            IntNe(x, y) => x.ev(run, rc) != y.ev(run, rc),
            IntLt(x, y) => x.ev(run, rc) < y.ev(run, rc),
            IntGt(x, y) => x.ev(run, rc) > y.ev(run, rc),
            IntLe(x, y) => x.ev(run, rc) <= y.ev(run, rc),
            IntGe(x, y) => x.ev(run, rc) >= y.ev(run, rc),
            _ => panic!(),
        }
    }
}

impl<A: Allocator + Default> BoolExp<A> {
    pub fn from(exp: &BoolExp<Local>, _src: &[u8]) -> Self {
        match exp {
            BoolExp::Bool(x) => BoolExp::Bool(*x),
            _ => todo!(),
        }
    }
}

/// Integer Expression.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum IntExp<A: Allocator + Default> {
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

impl<A: Allocator + Default> Eval<i64> for IntExp<A> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> i64 {
        use IntExp::*;
        match self {
            Int(x) => *x,
            Local(x) => {
                let ix = run.stack.len() - (x + 1);
                run.stack[ix].int()
            }
            Add(lhs, rhs) => lhs.ev(run, rc) + rhs.ev(run, rc),
            Sub(lhs, rhs) => lhs.ev(run, rc) - rhs.ev(run, rc),
            Mul(lhs, rhs) => lhs.ev(run, rc) * rhs.ev(run, rc),
            Div(lhs, rhs) => lhs.ev(run, rc) / rhs.ev(run, rc),
            Rem(lhs, rhs) => lhs.ev(run, rc) % rhs.ev(run, rc),
            _ => panic!(),
        }
    }
}

impl<A: Allocator + Default> IntExp<A> {
    /// Convert from Local allocator.
    pub fn from(exp: &IntExp<Local>, _src: &[u8]) -> Self {
        match exp {
            IntExp::Int(x) => IntExp::Int(*x),
            IntExp::Local(x) => IntExp::Local(*x),
            _ => todo!(),
        }
    }
}

/// String Expression.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum StrExp<A: Allocator + Default> {
    #[default]
    None,
    Local(usize),
    Col(usize),
    Str(GString),
    StrPos(SrcPos),
    Concat(BoxA<StrExp<A>, A>, BoxA<StrExp<A>, A>),
}

impl<A: Allocator + Default> Eval<LString> for StrExp<A> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> LString {
        match self {
            StrExp::None => panic!(),
            StrExp::Local(_) => todo!(),
            StrExp::Col(_) => todo!(),
            StrExp::Str(x) => LString::from(x.as_str()),
            StrExp::StrPos(x) => LString::from(x.sstr(run.source)),
            StrExp::Concat(lhs, rhs) => {
                let mut lhs = lhs.ev(run, rc);
                let rhs = rhs.ev(run, rc);
                lhs.push_str(&rhs);
                lhs
            }
        }
    }
}

impl<A: Allocator + Default> StrExp<A> {
    /// Convert from Local allocator.
    pub fn from(exp: &StrExp<Local>, src: &[u8]) -> Self {
        match exp {
            StrExp::Str(x) => StrExp::Str(GString::from(x.as_str())),
            StrExp::Local(x) => StrExp::Local(*x),
            StrExp::StrPos(x) => StrExp::Str(GString::from(x.sstr(src))),
            _ => todo!(),
        }
    }

    pub fn show(&self, sr: &mut SRun) -> Result<(), std::fmt::Error> {
        match self {
            StrExp::Str(x) => {
                sr.output.push_str("'");
                sr.output.push_str(x.as_str());
                sr.output.push_str("'");
            }
            _ => todo!(),
        }
        Ok(())
    }
}

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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum Exp<A: Allocator + Default> {
    #[default]
    None,
    Bool(BoolExp<A>),
    Int(IntExp<A>),
    Str(StrExp<A>),
    /// Name (unresolved).
    Name(SrcPos),
    Local(usize),
    Col(usize),
    Binary(Operator, BoxA<Exp<A>, A>, BoxA<Exp<A>, A>),

    /// Function call (unresolved). Schema, fname, args.
    FnCallByName(SrcPos, SrcPos, VecA<Exp<A>, A>),

    /// Function call (resolved). Function id and args.
    FnCall(usize, VecA<Exp<A>, A>),
    /// Built-in call. Build-in operation and args.
    CallBuiltin(Builtin, VecA<Exp<A>, A>),
}

impl<A: Allocator + Default> Eval<Value> for Exp<A> {
    fn ev<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> Value {
        match self {
            Exp::Bool(x) => Value::Bool(x.ev(run, rc)),
            Exp::Int(x) => Value::Int(x.ev(run, rc)),
            Exp::Str(x) => Value::String(LRc::new(x.ev(run, rc))),
            Exp::Local(x) => {
                let ix = run.stack.len() - (x + 1);
                run.stack[ix].clone()
            }
            Exp::Col(x) => rc.item(*x, run.ps),
            Exp::Binary(op, x, y) => {
                let x = x.ev(run, rc);
                let y = y.ev(run, rc);
                op.eval(&x, &y)
            }
            Exp::FnCall(f, args) => {
                let f = run.call_init(*f);
                let save = run.stack.len();
                for e in args {
                    let v = e.ev(run, rc);
                    run.stack.push(v);
                }
                execute_block(&f.block, run);
                run.stack.truncate(save);
                run.stack.pop().unwrap() // Pop return value.
            }
            Exp::CallBuiltin(bi, args) => {
                for e in args {
                    let v = e.ev(run, rc);
                    run.stack.push(v);
                }
                bi.eval(run)
            }
            _ => panic!(),
        }
    }
}

impl<A: Allocator + Default> Exp<A> {
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
            CallBuiltin(bi, args) => {
                let args = gvals(args, src);
                CallBuiltin(*bi, args)
            }
            _ => todo!(),
        }
    }

    /// Encode for execution.
    /// Eliminate Exp::Binary expressions, changing them to type specific Bool, Int or Str expressions.
    pub fn encode(&mut self) {
        // use std::ops::DerefMut;
        use Exp::*;
        match self {
            Binary(op, x, y) => {
                x.encode();
                y.encode();
                let re = match (op, &mut **x, &mut **y) {
                    (op, Bool(x), Bool(y)) => {
                        let x = BoxA::new(std::mem::take(x));
                        let y = BoxA::new(std::mem::take(y));
                        match op {
                            Operator::And => Bool(BoolExp::And(x, y)),
                            Operator::Or => Bool(BoolExp::Or(x, y)),
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
                    _ => { return; }
                };
                *self = re;
            }
            FnCall(_fid, args) => {
                for e in args
                {
                    e.encode();
                }
            }
            _ => {}
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
            Bool(BoolExp::Bool(x)) => write!(&mut sr.output, "{}", x)?,
            Int(IntExp::Int(x)) => write!(&mut sr.output, "{}", x)?,
            Str(x) => x.show(sr)?,
            Local(x) => sr.write_name(*x),
            Col(x) => sr.write_col_name(*x),
            Binary(op, x, y) => {
                let p = op.precedence();
                if p < pp || p == pp && right {
                    sr.output.push_str("(");
                }
                x.show_prec(sr, p, false)?;
                write!(&mut sr.output, " {} ", op)?;
                y.show_prec(sr, p, true)?;
                if p < pp || p == pp && right {
                    sr.output.push_str(")");
                }
            }
            FnCall(f, args) => {
                sr.write_fn_name(*f);
                Self::show_args(args, sr)?;
            }
            CallBuiltin(bi, args) => {
                write!(&mut sr.output, "sys.{:?}", bi)?;
                Self::show_args(args, sr)?;
            }
            _ => panic!(),
        }
        Ok(())
    }

    /// Show args.
    fn show_args(args: &[Exp<A>], sr: &mut SRun) -> Result<(), std::fmt::Error> {
        sr.output.push('(');
        let save = sr.aos;
        sr.aos += 1;
        for (i, e) in args.iter().enumerate() {
            if i > 0 {
                sr.output.push_str(", ");
            }
            e.show(sr)?;
            sr.aos += 1;
        }
        sr.output.push(')');
        sr.aos = save;
        Ok(())
    }
}

pub fn test_new_exp(ps: &mut PageSet) {
    let i1 = LBox::new(IntExp::Int(100));
    let i2 = LBox::new(IntExp::Int(5));
    let i3 = LBox::new(IntExp::Int(2));
    let exp1 = LBox::new(IntExp::Add(i1, i2));
    let exp3 = IntExp::Mul(exp1, i3);

    println!("exp3={:?}", exp3);

    let dict = Dict::new();
    let mut output = LVec::new();
    let mut run = Run {
        stack: LVec::new(),
        dict: &dict,
        ps,
        source: b"",
        output: &mut output,
    };

    let result = exp3.eval(&mut run);
    println!("result ={}", result);
}

/**/

/*
impl<A,AT> TExp<T> for Add<A,AT>
where
    A: Allocator + Default,
    AT: TExp<T>,
    T: ops::Add<Output = T>,
{
    fn eval(&self, run: &mut Run) -> T {
        self.0.eval(run) + self.1.eval(run)
    }
}
*/
