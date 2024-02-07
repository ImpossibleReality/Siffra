use crate::error::SiffraExecutionError;
use crate::grammar::Span;
use crate::representations::Compound;
use crate::representations::{Dimension, Quantity};
use crate::representations::{Expression, Float, Value};
use crate::siffra_try;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLine {
    Comment,
    Expression(ParsedExpr),
    Variable(String, ParsedExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedUnit {
    pub name: String,
    pub chemical: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDimension {
    pub numerator: Vec<(ParsedUnit, i32)>,
    pub denominator: Vec<(ParsedUnit, i32)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedExpr {
    Number {
        value: String,
        units: Option<ParsedDimension>,
        span: Span,
    },
    Variable {
        name: String,
        span: Span,
    },
    FunctionCall {
        name: String,
        args: Vec<ParsedExpr>,
        base: Option<Box<ParsedExpr>>,
        span: Span,
        function_span: Span,
    },
    UnOpPre {
        op: OpPre,
        rhs: Box<ParsedExpr>,
        span: Span,
    },
    UnOpPost {
        lhs: Box<ParsedExpr>,
        op: OpPost,
        span: Span,
    },
    BinOp {
        lhs: Box<ParsedExpr>,
        op: Op,
        rhs: Box<ParsedExpr>,
        span: Span,
    },
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum OpPre {
    Negate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpPost {
    Factorial,
    Percent,
    Convert(ParsedDimension),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Exponent,
}

impl TryFrom<ParsedDimension> for Dimension {
    type Error = SiffraExecutionError;

    fn try_from(dimension: ParsedDimension) -> Result<Self, SiffraExecutionError> {
        let mut quantities = Vec::new();

        for (unit, power) in dimension.numerator {
            if !(unit.name == "unitless" || unit.name == "number") {
                if let Some(chemical) = unit.chemical {
                    let compound = siffra_try!(
                        Compound::parse(chemical.as_str()).ok_or(()),
                        "Chemical Error",
                        "Error parsing chemical compound",
                        Some(unit.span)
                    );
                    quantities.push((
                        siffra_try!(
                            Quantity::from_str(unit.name.as_str()),
                            "Unit Error",
                            format!("Unit '{}' not defined", unit.name),
                            Some(unit.span)
                        )
                        .with_chemical(compound),
                        Float::from(power),
                    ));
                } else {
                    quantities.push((
                        siffra_try!(
                            Quantity::from_str(unit.name.as_str()),
                            "Unit Error",
                            format!("Unit '{}' not defined", unit.name),
                            Some(unit.span)
                        ),
                        Float::from(power),
                    ));
                }
            }
        }

        for (unit, power) in dimension.denominator {
            if !(unit.name == "unitless" || unit.name == "number") {
                if let Some(chemical) = unit.chemical {
                    let compound = siffra_try!(
                        Compound::parse(chemical.as_str()).ok_or(()),
                        "Syntax Error",
                        "Error parsing compound",
                        Some(unit.span)
                    );
                    quantities.push((
                        siffra_try!(
                            Quantity::from_str(unit.name.as_str()),
                            "Syntax Error",
                            "Error parsing quantity",
                            Some(unit.span)
                        )
                        .with_chemical(compound),
                        Float::from(-power),
                    ));
                } else {
                    quantities.push((
                        siffra_try!(
                            Quantity::from_str(unit.name.as_str()),
                            "Syntax Error",
                            "Error parsing quantity",
                            Some(unit.span)
                        ),
                        Float::from(-power),
                    ));
                }
            }
        }

        Ok(Dimension::new(quantities))
    }
}

impl TryFrom<ParsedExpr> for Expression {
    type Error = SiffraExecutionError;

    fn try_from(value: ParsedExpr) -> Result<Self, Self::Error> {
        match value {
            ParsedExpr::Number { value, units, span } => {
                let dimension = match units {
                    Some(units) => Some(Dimension::try_from(units)?),
                    None => None,
                };
                let num = siffra_try!(Float::parse(&*value), "Error parsing number", Some(span));

                Ok(Expression::constant(Value::new(num, dimension)).with_span(span))
            }
            ParsedExpr::Variable { name, span } => Ok(Expression::variable(name).with_span(span)),
            ParsedExpr::FunctionCall {
                name,
                args,
                base,
                span,
                function_span
            } => {
                let mut expressions = Vec::with_capacity(args.len() + 1);

                if let Some(base) = base {
                    expressions.push(Expression::try_from(*base)?)
                }

                for arg in args {
                    expressions.push(Expression::try_from(arg)?);
                }

                Ok(Expression::function_call(name, expressions).with_span(span).with_function_name_span(function_span))
            }
            ParsedExpr::UnOpPre { op, rhs, span } => {
                let rhs = Box::new(Expression::try_from(*rhs)?);

                match op {
                    OpPre::Negate => Ok(Expression::multiply(
                        *rhs,
                        Expression::constant(Value::from(-1.0)),
                    )
                    .with_span(span)),
                }
            }
            ParsedExpr::UnOpPost { lhs, op, span } => {
                let lhs = Box::new(Expression::try_from(*lhs)?);

                match op {
                    OpPost::Factorial => Ok(Expression::function_call(
                        "factorial".to_string(),
                        vec![*lhs],
                    )
                    .with_span(span)),
                    OpPost::Percent => Ok(Expression::divide(
                        *lhs,
                        Expression::constant(Value::from(100.0)),
                    )
                    .with_span(span)),
                    OpPost::Convert(dimension) => {
                        Ok(Expression::convert(*lhs, Dimension::try_from(dimension)?)
                            .with_span(span))
                    }
                }
            }
            ParsedExpr::BinOp { lhs, op, rhs, span } => {
                let lhs = Box::new(Expression::try_from(*lhs)?);
                let rhs = Box::new(Expression::try_from(*rhs)?);

                match op {
                    Op::Add => Ok(Expression::add(*lhs, *rhs).with_span(span)),
                    Op::Subtract => Ok(Expression::subtract(*lhs, *rhs).with_span(span)),
                    Op::Multiply => Ok(Expression::multiply(*lhs, *rhs).with_span(span)),
                    Op::Divide => Ok(Expression::divide(*lhs, *rhs).with_span(span)),
                    Op::Exponent => Ok(Expression::exponent(*lhs, *rhs).with_span(span)),
                }
            }
        }
    }
}
