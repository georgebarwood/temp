use serde::*;
use tablestg::*;

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
        } else if *self == Operator::Concat {
                    if let Value::String(x) = &x {
                        if let Value::String(y) = &y {
                            concat(x, y)
                        } else {
                            let temp = val_to_str(y);
                            concat(x, &temp)
                        }
                    } else {
                        let temp = val_to_str(x);
                        if let Value::String(y) = &y {
                            concat(&temp, y)
                        } else {
                            let temp2 = val_to_str(y);
                            concat(&temp, &temp2)
                        } 
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

pub fn val_to_str(x: &Value) -> LString {
    use std::fmt::Write;
    let mut result = LString::new();
    match x {
        Value::String(s) => result.push_str(s),
        Value::Int(x) => write!(result, "{}", x).unwrap(),
        Value::Bool(x) => write!(result, "{}", x).unwrap(),
        Value::Float(x) => write!(result, "{}", x.0).unwrap(),
        // Value::Binary(x) => util::to_hex(&mut result, x),
        _ => panic!(),
    }
    result
}
