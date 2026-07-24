use crate::*;
use serde::*;

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
    Concat, // string only for now, should support binary as well at some point.
    And,
    Or,
}

use std::fmt::Display;
use std::fmt::Formatter;
impl Display for Operator {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        use Operator::*;
        let x = match self {
            None => "???",
            Equal => "=",
            NotEqual => "!=",
            Greater => ">",
            Less => "<",
            GreaterEqual => ">=",
            LessEqual => "<=",
            Plus => "+",
            Minus => "-",
            Multiply => "*",
            Divide => "/",
            Remainder => "%",
            Concat => "|",
            And => "and",
            Or => "or",
        };
        f.write_str(x)?;
        Ok(())
    }
}

impl Operator {
    pub fn yields_bool(self) -> bool {
        use Operator::*;
        matches!(
            self,
            Equal | NotEqual | Greater | Less | GreaterEqual | LessEqual | And | Or
        )
    }

    pub fn precedence(&self) -> u8 {
        use Operator::*;
        match self {
            Concat => 1,
            Or => 2,
            And => 3,

            Equal | NotEqual | Less | Greater | LessEqual | GreaterEqual => 4,
            Plus | Minus => 5,
            Multiply | Divide | Remainder => 6,
            None => 0,
        }
    }

    /*
    /// This may not be called any more.
    pub fn eval(&self, x: &Value, y: &Value) -> Value {
        use Operator::*;

        // println!("Operator::eval self={:?}", self);
        
        if let Value::Int(x) = &x
            && let Value::Int(y) = &y
        {
            match self {
                Equal => Value::Bool(x == y),
                NotEqual => Value::Bool(x != y),
                Less => Value::Bool(x < y),
                Greater => Value::Bool(x > y),
                LessEqual => Value::Bool(x <= y),
                GreaterEqual => Value::Bool(x >= y),

                Plus => Value::Int(x + y),
                Minus => Value::Int(x - y),
                Multiply => Value::Int(x * y),
                Divide => Value::Int(x / y),
                Remainder => Value::Int(x % y),
                _ => todo!(),
            }
        } else if let Value::Bool(x) = &x
            && let Value::Bool(y) = &y
        {
            match self {
                And => Value::Bool(*x && *y),
                Or => Value::Bool(*x || *y),
                _ => todo!(),
            }
        } else if *self == Concat {
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
    */
}

/*
/// Concatenate strings.
fn concat(x: &str, y: &str) -> Value {
    let mut s = LString::with_capacity(x.len() + y.len());
    s.push_str(x);
    s.push_str(y);
    let s = LRc::new(s);
    Value::String(s)
}
*/

/// Convert value of any kind to string ( but binary not yet done ).
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
