use crate::error::SiffraExecutionError;
use crate::parser::Span;
use crate::representations::Compound;
use crate::representations::{Dimension, Unit};
use crate::representations::{Expression, Float, PercentageOperator, Value};
use crate::siffra_try;
use std::ops::Neg;

/// Parser-level aliases for the unscaled, unitless scalar dimension.
pub const SCALAR_UNIT_ALIASES: &[&str] = &["unitless", "number", "scalar", "#"];

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLine {
    Comment,
    /// A `#`-prefixed title line. Carries the title with the marker and
    /// surrounding whitespace stripped.
    Header(String),
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
    pub numerator: Vec<(ParsedUnit, String)>,
    pub denominator: Vec<(ParsedUnit, String)>,
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
    PercentageOn,
    PercentageOff,
    PercentageRelativeOf,
    PercentageRelativeOn,
    PercentageRelativeOff,
    PercentageOfWhat,
    PercentageOnWhat,
    PercentageOffWhat,
}

impl TryFrom<ParsedDimension> for Dimension {
    type Error = SiffraExecutionError;

    fn try_from(dimension: ParsedDimension) -> Result<Self, SiffraExecutionError> {
        let mut quantities = Vec::new();

        for (unit, power) in dimension.numerator {
            let power = siffra_try!(
                Float::parse(&power),
                "Error parsing unit power",
                Some(unit.span)
            );
            if !SCALAR_UNIT_ALIASES.contains(&unit.name.as_str()) {
                if let Some(chemical) = unit.chemical {
                    let compound = siffra_try!(
                        Compound::parse(chemical.as_str()).ok_or(()),
                        "Chemical Error",
                        format!("Invalid chemical formula '{chemical}'"),
                        Some(unit.span)
                    );
                    quantities.push((
                        siffra_try!(
                            Unit::from_str(unit.name.as_str()),
                            "Unit Error",
                            format!("Unit '{}' not defined", unit.name),
                            Some(unit.span)
                        )
                        .with_chemical(compound),
                        power.clone(),
                    ));
                } else {
                    quantities.push((
                        siffra_try!(
                            Unit::from_str(unit.name.as_str()),
                            "Unit Error",
                            format!("Unit '{}' not defined", unit.name),
                            Some(unit.span)
                        ),
                        power.clone(),
                    ));
                }
            }
        }

        for (unit, power) in dimension.denominator {
            let power = siffra_try!(
                Float::parse(&power),
                "Error parsing unit power",
                Some(unit.span)
            );
            if !SCALAR_UNIT_ALIASES.contains(&unit.name.as_str()) {
                if let Some(chemical) = unit.chemical {
                    let compound = siffra_try!(
                        Compound::parse(chemical.as_str()).ok_or(()),
                        "Chemical Error",
                        format!("Invalid chemical formula '{chemical}'"),
                        Some(unit.span)
                    );
                    quantities.push((
                        siffra_try!(
                            Unit::from_str(unit.name.as_str()),
                            "Unit Error",
                            format!("Unit '{}' not defined", unit.name),
                            Some(unit.span)
                        )
                        .with_chemical(compound),
                        power.clone().neg(),
                    ));
                } else {
                    quantities.push((
                        siffra_try!(
                            Unit::from_str(unit.name.as_str()),
                            "Unit Error",
                            format!("Unit '{}' not defined", unit.name),
                            Some(unit.span)
                        ),
                        power.clone().neg(),
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
                function_span,
            } => {
                let mut expressions = Vec::with_capacity(args.len() + 1);

                if let Some(base) = base {
                    expressions.push(Expression::try_from(*base)?)
                }

                for arg in args {
                    expressions.push(Expression::try_from(arg)?);
                }

                Ok(Expression::function_call(name, expressions)
                    .with_span(span)
                    .with_function_name_span(function_span))
            }
            ParsedExpr::UnOpPre { op, rhs, span } => {
                let rhs = Box::new(Expression::try_from(*rhs)?);

                match op {
                    OpPre::Negate => Ok(Expression::negate(*rhs).with_span(span)),
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
                    OpPost::Percent => Ok(Expression::percent(*lhs).with_span(span)),
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
                    Op::PercentageOn => {
                        Ok(
                            Expression::percentage_operation(*lhs, *rhs, PercentageOperator::On)
                                .with_span(span),
                        )
                    }
                    Op::PercentageOff => {
                        Ok(
                            Expression::percentage_operation(*lhs, *rhs, PercentageOperator::Off)
                                .with_span(span),
                        )
                    }
                    Op::PercentageRelativeOf => Ok(Expression::percentage_operation(
                        *lhs,
                        *rhs,
                        PercentageOperator::RelativeOf,
                    )
                    .with_span(span)),
                    Op::PercentageRelativeOn => Ok(Expression::percentage_operation(
                        *lhs,
                        *rhs,
                        PercentageOperator::RelativeOn,
                    )
                    .with_span(span)),
                    Op::PercentageRelativeOff => Ok(Expression::percentage_operation(
                        *lhs,
                        *rhs,
                        PercentageOperator::RelativeOff,
                    )
                    .with_span(span)),
                    Op::PercentageOfWhat => Ok(Expression::percentage_operation(
                        *lhs,
                        *rhs,
                        PercentageOperator::OfWhat,
                    )
                    .with_span(span)),
                    Op::PercentageOnWhat => Ok(Expression::percentage_operation(
                        *lhs,
                        *rhs,
                        PercentageOperator::OnWhat,
                    )
                    .with_span(span)),
                    Op::PercentageOffWhat => Ok(Expression::percentage_operation(
                        *lhs,
                        *rhs,
                        PercentageOperator::OffWhat,
                    )
                    .with_span(span)),
                }
            }
        }
    }
}
