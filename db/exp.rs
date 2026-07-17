use crate::*;
use pstd::{BoxA, StringA, VecA, alloc::Allocator};
use serde::*;

/// Position of string in source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrPos {
    pub start: usize,
    pub end: usize,
}

impl StrPos {
    pub fn str<'a>(&self, src: &'a [u8]) -> &'a str {
        tos(&src[self.start..self.end])
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
    SrcString(StrPos),
    /// Unresolved name
    Name(StrPos),
    /// Column number
    Col(usize),
    /// Local variable.
    Local(usize),
    /// Binary expression.
    Binary(Operator, BoxA<Exp<A>, A>, BoxA<Exp<A>, A>),
    /// Function call (unresolved). Schema, fname, args.
    FnCallByName(StrPos, StrPos, VecA<Exp<A>, A>),
    /// Function call (resolved). Function id and args.
    FnCall(usize, VecA<Exp<A>, A>),
}

impl<A: Allocator + Default> Exp<A> {
    pub fn eval(&self, run: &mut Run) -> Value {
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
            Binary(op, x, y) => {
                let x = x.eval(run);
                let y = y.eval(run);
                op.eval(&x, &y)
            }
            FnCall(f, args) => {
                // Push default value for result onto stack.
                let f = &run.dict.main.funcs[*f];
                let def = f.ret.default_value();
                run.stack.push(def);

                let save = run.stack.len();
                for e in args {
                    let v = e.eval(run);
                    run.stack.push(v);
                }
                // Execute the function.
                execute_fn(f, run);

                run.stack.truncate(save);
                run.stack.pop().unwrap() // Pop return value.
            }
            Col(_) | Name(_) | FnCallByName(_, _, _) => panic!(),
        }
    }

    pub fn eval_lr(&self, run: &mut Run, lr: &mut LazyRow) -> Value {
        use Exp::*;
        match self {
            Col(x) => lr.item(*x, run.ps),
            Binary(op, x, y) => {
                let x = x.eval_lr(run, lr);
                let y = y.eval_lr(run, lr);
                op.eval(&x, &y)
            }
            FnCall(f, args) => {
                // Push default value for result onto stack.
                let f = &run.dict.main.funcs[*f];
                let def = f.ret.default_value();
                run.stack.push(def);

                let save = run.stack.len();
                for e in args {
                    let v = e.eval_lr(run, lr);
                    // println!("func arg={:?}", v);
                    run.stack.push(v);
                }
                // Execute the function.
                execute_fn(f, run);

                run.stack.truncate(save);
                run.stack.pop().unwrap() // Pop return value.
            }
            _ => self.eval(run),
        }
    }

    pub fn eval_vals(&self, run: &mut Run, vals: &[Value]) -> Value {
        use Exp::*;
        match self {
            Col(x) => vals[*x].clone(),
            Binary(op, x, y) => {
                let x = x.eval_vals(run, vals);
                let y = y.eval_vals(run, vals);
                op.eval(&x, &y)
            }
            FnCall(f, args) => {
                // Push default value for result onto stack.
                let f = &run.dict.main.funcs[*f];
                let def = f.ret.default_value();
                run.stack.push(def);

                let save = run.stack.len();
                for e in args {
                    let v = e.eval_vals(run, vals);
                    run.stack.push(v);
                }
                // Execute the function.
                execute_fn(f, run);

                run.stack.truncate(save);
                run.stack.pop().unwrap() // Pop return value.
            }
            _ => self.eval(run),
        }
    }

    /// Convert from LExp
    pub fn from(exp: &LExp, src: &[u8]) -> Self {
        use Exp::*;
        match exp {
            Bool(x) => Bool(*x),
            Int(x) => Int(*x),
            SrcString(x) => String(StringA::from(x.str(src))),  
            String(x) => String(StringA::from(x.as_str())),
            Col(x) => Col(*x),
            Local(x) => Local(*x),
            Binary(op, lhs, rhs) => {
                let lhs = BoxA::new(Self::from(lhs, src));
                let rhs = BoxA::new(Self::from(rhs, src));
                Binary(*op, lhs, rhs)
            }
            FnCall(fid, args) => {
                let args = gvals(args, src);
                FnCall(*fid, args)
            }
            Name(_) | FnCallByName(_, _, _) => panic!()
        }
    }
}
