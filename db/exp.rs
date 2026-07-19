use crate::*;
use serde::*;

/// Position of string in source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrcPos {
    pub start: usize,
    pub end: usize,
}

impl SrcPos {
    pub fn str<'a>(&self, src: &'a [u8]) -> &'a str {
        tos(&src[self.start..self.end])
    }
}

/// No row context.
struct NoRowContext;
impl RowContext for NoRowContext {
    fn item(&mut self, _i: usize, _ps: &mut PageSet) -> Value {
        panic!()
    }
}

/// Row context that is list of values.
struct ValsRowContext<'a> {
    vals: &'a [Value],
}

impl<'a> RowContext for ValsRowContext<'a> {
    fn item(&mut self, item: usize, _ps: &mut PageSet) -> Value {
        self.vals[item].clone()
    }
}

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

use std::fmt::Write;

impl<A> Exp<A>
where
    A: Allocator + Default,
{
    /// Should have a precedence arg that determines if brackets are needed.
    pub fn show(&self, sr: &mut SRun) -> Result<(), std::fmt::Error> {
        use Exp::*;
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
                sr.output.push_str("(");
                x.show(sr)?;
                write!(&mut sr.output, " {} ", op)?;
                y.show(sr)?;
                sr.output.push_str(")");
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

impl<A: Allocator + Default> Exp<A> {
    /// Evaluate the expression using specified Run and RowContext.
    pub fn eval_rc<C: RowContext>(&self, run: &mut Run, rc: &mut C) -> Value {
        use Exp::*;
        match self {
            Bool(x) => Value::Bool(*x),
            Int(x) => Value::Int(*x),
            SrcString(x) => {
                let s = x.str(run.source);
                Value::String(LRc::new(LString::from(s)))
            }
            String(x) => Value::String(LRc::new(LString::from(x.as_ref()))),
            Local(x) => {
                let ix = run.stack.len() - (x + 1);
                run.stack[ix].clone()
            }
            Col(x) => rc.item(*x, run.ps),
            Binary(op, x, y) => {
                let x = x.eval_rc(run, rc);
                let y = y.eval_rc(run, rc);
                op.eval(&x, &y)
            }
            FnCall(f, args) => {
                let f = run.call_init(*f);
                let save = run.stack.len();
                for e in args {
                    let v = e.eval_rc(run, rc);
                    run.stack.push(v);
                }
                execute_block(&f.block, run);
                run.stack.truncate(save);
                run.stack.pop().unwrap() // Pop return value.
            }
            CallBuiltin(bi, args) => {
                for e in args {
                    let v = e.eval_rc(run, rc);
                    run.stack.push(v);
                }
                bi.eval(run)
            }
            Name(_) | FnCallByName(_, _, _) => panic!(),
        }
    }

    /// Evaluate the expression, no row context.
    pub fn eval(&self, run: &mut Run) -> Value {
        self.eval_rc(run, &mut NoRowContext)
    }

    /// Evaluate the expression using specified row values.
    pub fn eval_vals(&self, run: &mut Run, vals: &[Value]) -> Value {
        let mut vc = ValsRowContext { vals };
        self.eval_rc(run, &mut vc)
    }

    /// Convert from LExp
    pub fn from(exp: &LExp, src: &[u8]) -> Self {
        use Exp::*;
        match exp {
            Bool(x) => Bool(*x),
            Int(x) => Int(*x),
            SrcString(x) => String(StringA::from(x.str(src))),
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
}
