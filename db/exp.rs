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

/// Parsed Expression (for shared stored functions).
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
    /// Function call (resolved).
    FnCall(usize, GVec<GExp>),
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
                let f = &run.dict.funcs[*f];
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
                let f = &run.dict.funcs[*f];
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
                let f = &run.dict.funcs[*f];
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
                let f = &run.dict.funcs[*f];
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
                let f = &run.dict.funcs[*f];
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
                let f = &run.dict.funcs[*f];
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
