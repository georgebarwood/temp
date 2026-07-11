use tablestg::*;

/// Parsed Expression.
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

/// Arithmetic and other binary operators.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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
}

impl<'a> Exp<'a> {
    pub fn eval(&self, locals: &'a [Value]) -> Value {
        match self {
            Exp::Bool(x) => Value::Bool(*x),
            Exp::Int(x) => Value::Int(*x),
            Exp::String(x) => Value::String(LRc::new(LString::from(*x))),
            Exp::Col(_) => panic!(),
            Exp::Local(x) => {
                let ix = locals.len() - (x + 1);
                locals[ix].clone()
            }
            Exp::Binary(op, x, y) => {
                let x: Value = x.eval(locals);
                let y: Value = y.eval(locals);
                if let Value::Int(x) = &x
                    && let Value::Int(y) = &y
                {
                    match op {
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
                    match op {
                        Operator::And => Value::Bool(*x && *y),
                        Operator::Or => Value::Bool(*x || *y),
                        _ => todo!(),
                    }
                } else if let Value::String(x) = &x
                    && let Value::String(y) = &y
                {
                    match op {
                        Operator::Concat => concat(x, y),
                        _ => todo!(),
                    }
                } else {
                    println!("self={:?}", self);
                    todo!()
                }
            }
            _ => {
                println!("self={:?}", self);
                todo!()
            }
        }
    }

    pub fn eval_lr(&self, locals: &'a [Value], lr: &mut LazyRow, ps: &mut PageSet) -> Value {
        match self {
            Exp::Bool(x) => Value::Bool(*x),
            Exp::Int(x) => Value::Int(*x),
            Exp::String(x) => Value::String(LRc::new(LString::from(*x))),
            Exp::Col(x) => lr.item(*x, ps),
            Exp::Local(x) => {
                let ix = locals.len() - (x + 1);
                locals[ix].clone()
            }
            Exp::Binary(op, x, y) => {
                let x: Value = x.eval_lr(locals, lr, ps);
                let y: Value = y.eval_lr(locals, lr, ps);
                if let Value::Int(x) = &x
                    && let Value::Int(y) = &y
                {
                    match op {
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
                    match op {
                        Operator::And => Value::Bool(*x && *y),
                        Operator::Or => Value::Bool(*x || *y),
                        _ => todo!(),
                    }
                } else if let Value::String(x) = &x
                    && let Value::String(y) = &y
                {
                    match op {
                        Operator::Concat => concat(x, y),
                        _ => todo!(),
                    }
                } else {
                    println!("self={:?}", self);
                    todo!()
                }
            }
            _ => {
                println!("self={:?}", self);
                todo!()
            }
        }
    }

    pub fn eval_vals(&self, locals: &[Value], vals: &[Value]) -> Value {
        match self {
            Exp::Bool(x) => Value::Bool(*x),
            Exp::Int(x) => Value::Int(*x),
            Exp::String(x) => Value::String(LRc::new(LString::from(*x))),
            Exp::Col(x) => vals[*x].clone(),
            Exp::Local(x) => {
                let ix = locals.len() - (x + 1);
                locals[ix].clone()
            }
            Exp::Binary(op, x, y) => {
                let x: Value = x.eval_vals(locals, vals);
                let y: Value = y.eval_vals(locals, vals);
                if let Value::Int(x) = &x
                    && let Value::Int(y) = &y
                {
                    match op {
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
                    match op {
                        Operator::And => Value::Bool(*x && *y),
                        Operator::Or => Value::Bool(*x || *y),
                        _ => todo!(),
                    }
                } else if let Value::String(x) = &x
                    && let Value::String(y) = &y
                {
                    match op {
                        Operator::Concat => concat(x, y),
                        _ => todo!(),
                    }
                } else {
                    println!("self={:?}", self);
                    todo!()
                }
            }
            _ => {
                println!("self={:?}", self);
                todo!()
            }
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
