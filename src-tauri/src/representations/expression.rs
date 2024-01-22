use crate::representations::dimension::{Dimension, Value};

#[derive(Debug, Clone)]
pub enum Expression {
    Constant(Value),
    Convert(Box<Expression>, Dimension),
    Variable(String),
    FunctionCall { name: String, args: Vec<Expression> },
    Multiply(Box<Expression>, Box<Expression>),
    Divide(Box<Expression>, Box<Expression>),
    Add(Box<Expression>, Box<Expression>),
    Subtract(Box<Expression>, Box<Expression>),
    Exponent(Box<Expression>, Box<Expression>),
}
