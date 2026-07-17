use crate::*;
use serde::*;

/// Parsed Expression (temporary, for local execution of batch).
#[derive(Debug)]
pub enum Exp<'a> {
    /// Bool constant
    Bool(bool),
    /// Integer constant
    Int(i64),
    /// String literal
    String(&'a str),
    /// Unresolved name
    Name(&'a str),
    /// Column number
    Col(usize),
    /// Local variable.
    Local(usize),
    /// Binary expression.
    Binary(Operator, LBox<Exp<'a>>, LBox<Exp<'a>>),
    /// Function call (unresolved). Schema, fname, args.
    FnCallByName(&'a str, &'a str, LVec<Exp<'a>>),
    /// Function call (resolved). Function id and args.
    FnCall(usize, LVec<Exp<'a>>),
}

impl<'a> Exp<'a> {
    pub fn eval(&self, run: &mut Run) -> Value {
        use Exp::*;
        match self {
            Bool(x) => Value::Bool(*x),
            Int(x) => Value::Int(*x),
            String(x) => Value::String(LRc::new(LString::from(*x))),
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
}

/*

impl GExp {
    pub fn eval(&self, run: &mut Run) -> Value {
        use GExp::*;
        match self {
            Bool(x) => Value::Bool(*x),
            Int(x) => Value::Int(*x),
            String(x) => Value::String(LRc::new(LString::from(&**x))),
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
            Col(_) => panic!(),
        }
    }
    pub fn eval_lr(&self, run: &mut Run, lr: &mut LazyRow) -> Value {
        use GExp::*;
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
        use GExp::*;
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
    pub fn walk<F>(&mut self, f:&mut F) where F: FnMut(&mut GExp) {
        f(self);
        match self {
            GExp::Binary(_,lhs,rhs) => {
                lhs.walk(f);
                rhs.walk(f);
            }
            GExp::FnCall(_,args) => {
                for e in args { e.walk(f) }
            }
            _ => {}
        }
    }
}

impl GExp {
    pub fn from(exp: &Exp) -> Self {
        match exp {
            Exp::Bool(x) => GExp::Bool(*x),
            Exp::Int(x) => GExp::Int(*x),
            Exp::String(x) => GExp::String(GString::from(*x)),
            Exp::Col(x) => GExp::Col(*x),
            Exp::Local(x) => GExp::Local(*x),
            Exp::Binary(op, lhs, rhs) => {
                let lhs = GBox::new(GExp::from(lhs));
                let rhs = GBox::new(GExp::from(rhs));
                GExp::Binary(*op, lhs, rhs)
            }
            Exp::FnCall(fid, args) => {
                let args = gvals(args);
                GExp::FnCall(*fid, args)
            }
            Exp::Name(_) | Exp::FnCallByName(_, _, _) => panic!(),
        }
    }
}

use std::io::Write;

/// Instructions push names onto stack of names, etc.
pub struct N {
    pub w: LVec<u8>,
}

impl GExp
{
    fn _show(&self, n: &mut N) -> Result<(), std::io::Error>
    {
        match self {
           GExp::Bool(x) => write!(n.w, "{}", x)?,
           GExp::Int(x) => write!(n.w, " {} ", x)?,
           GExp::String(x) => write!(n.w, " {} ", x)?,
           GExp::Local(_x) => /*GExp::Local(*x),*/ todo!(), // Lookup name from run.name_stack

           GExp::FnCall(_fix, _args) =>
           {
               // Lookup name from drun.reverse_map.
           }
           _ => todo!(),
        }
        Ok(())
    }
}
*/

///////////////////////////////////

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GExp {
    /// Bool constant
    Bool(bool),
    /// Integer constant
    Int(i64),
    /// String literal
    String(GString),
    /// Column number
    Col(usize),
    /// Local variable.
    Local(usize),
    /// Binary expression.
    Binary(Operator, GBox<GExp>, GBox<GExp>),
    /// Function call (resolved). Function id and args.
    FnCall(usize, GVec<GExp>),
}

impl GExp {
    /// Convert Exp to GExp.
    pub fn from(exp: &Exp) -> GExp {
        match exp {
            Exp::Bool(x) => GExp::Bool(*x),
            Exp::Int(x) => GExp::Int(*x),
            Exp::String(x) => GExp::String(GString::from(*x)),
            Exp::Col(x) => GExp::Col(*x),
            Exp::Local(x) => GExp::Local(*x),
            Exp::Binary(op, lhs, rhs) => {
                let lhs = GBox::new(GExp::from(lhs));
                let rhs = GBox::new(GExp::from(rhs));
                GExp::Binary(*op, lhs, rhs)
            }
            Exp::FnCall(fid, args) => {
                let args = Self::genvals(args);
                GExp::FnCall(*fid, args)
            }
            _ => panic!(),
        }
    }

    fn genvals(list: &[Exp]) -> GVec<GExp> {
        let mut result = GVec::with_capacity(list.len());
        for e in list {
            result.push(GExp::from(e));
        }
        result
    }

    pub fn eval(&self, run: &mut Run) -> Value {
        use GExp::*;
        match self {
            Bool(x) => Value::Bool(*x),
            Int(x) => Value::Int(*x),
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
            _ => panic!(),
        }
    }

    pub fn eval_lr(&self, run: &mut Run, lr: &mut LazyRow) -> Value {
        use GExp::*;
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
        use GExp::*;
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
}
