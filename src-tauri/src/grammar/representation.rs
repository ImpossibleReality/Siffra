use crate::representations::{Dimension, Expression, Quantity, Value};
use num::{BigRational, FromPrimitive};
use std::str::FromStr;

#[derive(Debug)]
pub struct ParsedUnit {
    pub(crate) name: String,
    pub(crate) chemical: Option<String>,
}

#[derive(Debug)]
pub struct ParsedDimension {
    pub(crate) numerator: Vec<(ParsedUnit, i32)>,
    pub(crate) denominator: Vec<(ParsedUnit, i32)>,
}

#[derive(Debug)]
pub enum ParsedExpr {
    Number {
        value: f64,
        units: Option<ParsedDimension>,
    },
    Variable {
        name: String,
    },
    FunctionCall {
        name: String,
        args: Vec<ParsedExpr>,
        base: Option<Box<ParsedExpr>>,
    },
    UnOpPre {
        op: OpPre,
        rhs: Box<ParsedExpr>,
    },
    UnOpPost {
        lhs: Box<ParsedExpr>,
        op: OpPost,
    },
    BinOp {
        lhs: Box<ParsedExpr>,
        op: Op,
        rhs: Box<ParsedExpr>,
    },
}

#[derive(Debug)]
pub enum OpPre {
    Negate,
}

#[derive(Debug)]
pub enum OpPost {
    Factorial,
    Percent,
    Convert(ParsedDimension),
}

#[derive(Debug)]
pub enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Exponent,
}

impl TryFrom<ParsedDimension> for Dimension {
    type Error = ();

    fn try_from(dimension: ParsedDimension) -> Result<Self, ()> {
        let mut quantities = Vec::new();

        for (unit, power) in dimension.numerator {
            quantities.push((Quantity::from_str(unit.name.as_str())?, power));
        }

        for (unit, power) in dimension.denominator {
            quantities.push((Quantity::from_str(unit.name.as_str())?, -power));
        }

        Ok(Dimension::new(quantities))
    }
}

impl TryFrom<ParsedExpr> for Expression {
    type Error = ();

    fn try_from(value: ParsedExpr) -> Result<Self, ()> {
        match value {
            ParsedExpr::Number { value, units } => {
                let dimension = match units {
                    Some(units) => Some(Dimension::try_from(units)?),
                    None => None,
                };

                Ok(Expression::Constant(Value::new(
                    BigRational::from_f64(value).ok_or(())?,
                    dimension,
                )))
            }
            ParsedExpr::Variable { name } => Ok(Expression::Variable(name)),
            ParsedExpr::FunctionCall { name, args, base } => {
                let mut expressions = Vec::with_capacity(args.len() + 1);

                if let Some(base) = base {
                    expressions.push(Expression::try_from(*base)?)
                }

                for arg in args {
                    expressions.push(Expression::try_from(arg)?);
                }

                Ok(Expression::FunctionCall {
                    name,
                    args: expressions,
                })
            }
            ParsedExpr::UnOpPre { op, rhs } => {
                let rhs = Box::new(Expression::try_from(*rhs)?);

                match op {
                    OpPre::Negate => Ok(Expression::Multiply(
                        rhs,
                        Box::new(Expression::Constant(Value::from(-1.0))),
                    )),
                }
            }
            ParsedExpr::UnOpPost { lhs, op } => {
                let lhs = Box::new(Expression::try_from(*lhs)?);

                match op {
                    OpPost::Factorial => Ok(Expression::FunctionCall {
                        name: "factorial".to_string(),
                        args: vec![*lhs],
                    }),
                    OpPost::Percent => Ok(Expression::Divide(
                        lhs,
                        Box::new(Expression::Constant(Value::from(100.0))),
                    )),
                    OpPost::Convert(dimension) => {
                        Ok(Expression::Convert(lhs, Dimension::try_from(dimension)?))
                    }
                }
            }
            ParsedExpr::BinOp { lhs, op, rhs } => {
                let lhs = Box::new(Expression::try_from(*lhs)?);
                let rhs = Box::new(Expression::try_from(*rhs)?);

                match op {
                    Op::Add => Ok(Expression::Add(lhs, rhs)),
                    Op::Subtract => Ok(Expression::Subtract(lhs, rhs)),
                    Op::Multiply => Ok(Expression::Multiply(lhs, rhs)),
                    Op::Divide => Ok(Expression::Divide(lhs, rhs)),
                    Op::Exponent => Ok(Expression::Exponent(lhs, rhs)),
                }
            }
        }
    }
}
