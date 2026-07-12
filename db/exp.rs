use serde::*;
use tablestg::*;

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
}

/// Parsed Expression (for shared stored functions).
#[derive(Debug, Serialize, Deserialize)]
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
}

/// Arithmetic and other binary operators.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Operator {
    None,
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
    Plus,
    Minus,
    Multiply,
    Divide,
    Remainder,
    Concat,
    And,
    Or,
}

impl Operator {
    pub fn yields_bool(self) -> bool {
        use Operator::*;
        matches!(
            self,
            Equal | NotEqual | Greater | Less | GreaterEqual | LessEqual | And | Or
        )
    }
    pub fn eval(&self, x: &Value, y: &Value) -> Value {
        if let Value::Int(x) = &x
            && let Value::Int(y) = &y
        {
            match self {
                Operator::Equal => Value::Bool(x == y),
                Operator::NotEqual => Value::Bool(x != y),
                Operator::Less => Value::Bool(x < y),
                Operator::Greater => Value::Bool(x > y),
                Operator::LessEqual => Value::Bool(x <= y),
                Operator::GreaterEqual => Value::Bool(x >= y),

                Operator::Plus => Value::Int(x + y),
                Operator::Minus => Value::Int(x - y),
                Operator::Multiply => Value::Int(x * y),
                Operator::Divide => Value::Int(x / y),
                Operator::Remainder => Value::Int(x % y),
                _ => todo!(),
            }
        } else if let Value::Bool(x) = &x
            && let Value::Bool(y) = &y
        {
            match self {
                Operator::And => Value::Bool(*x && *y),
                Operator::Or => Value::Bool(*x || *y),
                _ => todo!(),
            }
        } else if let Value::String(x) = &x
            && let Value::String(y) = &y
        {
            match self {
                Operator::Concat => concat(x, y),
                _ => todo!(),
            }
        } else {
            println!("self={:?}", self);
            todo!()
        }
    }
}

fn concat(x: &str, y: &str) -> Value {
    let mut s = LString::with_capacity(x.len() + y.len());
    s.push_str(x);
    s.push_str(y);
    let s = LRc::new(s);
    Value::String(s)
}

impl<'a> Exp<'a> {
    pub fn eval(&self, locals: &'a [Value]) -> Value {
        use Exp::*;
        match self {
            Bool(x) => Value::Bool(*x),
            Int(x) => Value::Int(*x),
            String(x) => Value::String(LRc::new(LString::from(*x))),
            Local(x) => {
                let ix = locals.len() - (x + 1);
                locals[ix].clone()
            }
            Binary(op, x, y) => {
                let x = x.eval(locals);
                let y = y.eval(locals);
                op.eval(&x, &y)
            }
            Col(_) => panic!(),
            Name(_) => panic!(),
        }
    }

    pub fn eval_lr(&self, locals: &'a [Value], lr: &mut LazyRow, ps: &mut PageSet) -> Value {
        use Exp::*;
        match self {
            Col(x) => lr.item(*x, ps),
            Binary(op, x, y) => {
                let x = x.eval_lr(locals, lr, ps);
                let y = y.eval_lr(locals, lr, ps);
                op.eval(&x, &y)
            }
            _ => self.eval(locals),
        }
    }

    pub fn eval_vals(&self, locals: &[Value], vals: &[Value]) -> Value {
        use Exp::*;
        match self {
            Col(x) => vals[*x].clone(),
            Binary(op, x, y) => {
                let x = x.eval_vals(locals, vals);
                let y = y.eval_vals(locals, vals);
                op.eval(&x, &y)
            }
            _ => self.eval(locals),
        }
    }
}

impl GExp {
    pub fn eval(&self, locals: &[Value]) -> Value {
        use GExp::*;
        match self {
            Bool(x) => Value::Bool(*x),
            Int(x) => Value::Int(*x),
            String(x) => Value::String(LRc::new(LString::from(&**x))),
            Local(x) => {
                let ix = locals.len() - (x + 1);
                locals[ix].clone()
            }
            Binary(op, x, y) => {
                let x = x.eval(locals);
                let y = y.eval(locals);
                op.eval(&x, &y)
            }
            Col(_) => panic!(),
        }
    }
    pub fn eval_lr(&self, locals: &[Value], lr: &mut LazyRow, ps: &mut PageSet) -> Value {
        use GExp::*;
        match self {
            Col(x) => lr.item(*x, ps),
            Binary(op, x, y) => {
                let x = x.eval_lr(locals, lr, ps);
                let y = y.eval_lr(locals, lr, ps);
                op.eval(&x, &y)
            }
            _ => self.eval(locals),
        }
    }

    pub fn eval_vals(&self, locals: &[Value], vals: &[Value]) -> Value {
        use GExp::*;
        match self {
            Col(x) => vals[*x].clone(),
            Binary(op, x, y) => {
                let x = x.eval_vals(locals, vals);
                let y = y.eval_vals(locals, vals);
                op.eval(&x, &y)
            }
            _ => self.eval(locals),
        }
    }
}

impl GExp {
    pub fn from(exp: &Exp) -> Self {
        match exp {
            Exp::Bool(x) => GExp::Bool(*x),
            Exp::Int(x) => GExp::Int(*x),
            Exp::String(x) => GExp::String(GString::from(*x)),
            Exp::Name(_x) => panic!(),
            Exp::Col(x) => GExp::Col(*x),
            Exp::Local(x) => GExp::Local(*x),
            Exp::Binary(op, lhs, rhs) => {
                let lhs = GBox::new(GExp::from(lhs));
                let rhs = GBox::new(GExp::from(rhs));
                GExp::Binary(*op, lhs, rhs)
            }
        }
    }
}
