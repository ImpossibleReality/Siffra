use crate::error::SiffraExecutionError;
use crate::parser::Span;
use crate::representations::Compound;
use crate::representations::ladder;
use crate::representations::{
    ClockLiteral, Expression, Float, InstantKeyword, InstantLiteral, PercentageOperator, TimeZone,
    Value,
};
use crate::representations::{Dimension, Unit};
use crate::{siffra_error, siffra_try};
use std::ops::Neg;

pub const SCALAR_UNIT_ALIASES: &[&str] = &["unitless", "number", "scalar", "#"];

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedLine {
    Comment,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedDate {
    pub year: Option<i16>,
    pub month: i8,
    pub day: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedTime {
    pub hour: i8,
    pub minute: i8,
    pub second: Option<i8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedInstant {
    pub keyword: Option<InstantKeyword>,
    pub date: Option<ParsedDate>,
    pub time: Option<ParsedTime>,
    pub zone: Option<(String, Span)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedExpr {
    Number {
        value: String,
        units: Option<ParsedDimension>,
        span: Span,
    },
    /// A point on the calendar: date, datetime, zoned datetime, or keyword.
    Instant {
        instant: ParsedInstant,
        span: Span,
    },
    /// A mixed-unit literal: `5 ft 2 in`, `2 h 5 min 2 s`, `5'2"`. One value
    /// written across several units, largest first.
    Mixed {
        rungs: Vec<(String, ParsedUnit)>,
        span: Span,
    },
    /// A wall-clock reading, with the zone it is read in when one is named.
    ClockTime {
        time: ParsedTime,
        zone: Option<(String, Span)>,
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
    /// A mixed-unit conversion target: `to ft,in`, `-> [h, min, s]`. Each entry
    /// names one rung.
    Ladder(Vec<ParsedDimension>),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Op {
    Add,
    Subtract,
    Interval,
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

/// One rung of a conversion ladder. Rungs are plain units, so a compound or a
/// power in that position is a different request than the ladder syntax makes.
fn ladder_rung(dimension: ParsedDimension) -> Result<Unit, SiffraExecutionError> {
    let span = dimension.span;
    match Dimension::try_from(dimension)?.0.as_slice() {
        [(unit, power)] if *power == Float::from(1) => Ok(unit.clone()),
        _ => Err(siffra_error!(
            "Unit Error",
            "Each rung of a mixed-unit target is a single unit",
            Some(span)
        )),
    }
}

/// The value a mixed literal names: the rungs summed, read in the smallest of
/// them, remembering how it was written so it reads back the same way.
fn mixed_literal(
    rungs: Vec<(String, ParsedUnit)>,
    span: Span,
) -> Result<Value, SiffraExecutionError> {
    let mut units = Vec::with_capacity(rungs.len());
    let mut total = Float::from(0);

    for (amount, unit) in rungs {
        let parsed = siffra_try!(
            Unit::from_str(unit.name.as_str()),
            "Unit Error",
            format!("Unit '{}' not defined", unit.name),
            Some(unit.span)
        );
        let factor = siffra_try!(
            parsed.factor().ok_or(()),
            "Unit Error",
            format!("Unit '{}' has no known size", unit.name),
            Some(unit.span)
        );
        let amount = siffra_try!(Float::parse(&amount), "Error parsing number", Some(span));
        total = &total + &(&amount * &factor);
        units.push(parsed);
    }

    if let Err(error) = ladder::validate(&units) {
        return Err(siffra_error!("Unit Error", error.message(), Some(span)));
    }

    let smallest = units.last().expect("a mixed literal has rungs").clone();
    let factor = smallest.factor().expect("a checked rung has a size");
    Ok(Value::new(&total / &factor, Some(ladder::rung_dimension(&smallest))).with_ladder(units))
}

fn parse_zone(zone: Option<(String, Span)>) -> Result<Option<TimeZone>, SiffraExecutionError> {
    match zone {
        Some((name, span)) => Ok(Some(siffra_try!(
            TimeZone::parse(&name).ok_or(()),
            "Timezone Error",
            format!("'{name}' is not a known timezone"),
            Some(span)
        ))),
        None => Ok(None),
    }
}

fn checked_time(time: ParsedTime, span: Span) -> Result<(i8, i8, i8), SiffraExecutionError> {
    let second = time.second.unwrap_or(0);
    if time.hour > 23 || time.minute > 59 || second > 59 {
        return Err(siffra_error!(
            "Time Error",
            format!(
                "{:02}:{:02}:{:02} is not a valid time of day",
                time.hour, time.minute, second
            ),
            Some(span)
        ));
    }
    Ok((time.hour, time.minute, second))
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
                let num = siffra_try!(Float::parse(&value), "Error parsing number", Some(span));

                Ok(Expression::constant(Value::new(num, dimension)).with_span(span))
            }
            ParsedExpr::Instant { instant, span } => {
                let zone = parse_zone(instant.zone)?;
                let time = match instant.time {
                    Some(time) => Some(checked_time(time, span)?),
                    None => None,
                };
                Ok(Expression::instant(InstantLiteral {
                    keyword: instant.keyword,
                    date: instant.date.map(|date| (date.year, date.month, date.day)),
                    time,
                    has_seconds: instant.time.is_some_and(|time| time.second.is_some()),
                    zone,
                })
                .with_span(span))
            }
            ParsedExpr::Mixed { rungs, span } => {
                Ok(Expression::constant(mixed_literal(rungs, span)?).with_span(span))
            }
            ParsedExpr::ClockTime { time, zone, span } => Ok(Expression::clock(ClockLiteral {
                time: checked_time(time, span)?,
                has_seconds: time.second.is_some(),
                zone: parse_zone(zone)?,
            })
            .with_span(span)),
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
                    // Only some functions read a subscript, and for those the base
                    // is simply the first argument. Rejecting it here, rather than
                    // letting it become an argument, keeps `sin_2(x)` from being
                    // reported as a wrong argument count.
                    if let Some(function) = crate::registry::function(&name)
                        && matches!(function.base, crate::registry::Base::None)
                    {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} does not take a base"),
                            Some(function_span)
                        ));
                    }
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
                    OpPost::Ladder(rungs) => {
                        let units = rungs
                            .into_iter()
                            .map(ladder_rung)
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(Expression::convert_ladder(*lhs, units).with_span(span))
                    }
                }
            }
            ParsedExpr::BinOp { lhs, op, rhs, span } => {
                let lhs = Box::new(Expression::try_from(*lhs)?);
                let rhs = Box::new(Expression::try_from(*rhs)?);

                match op {
                    Op::Add => Ok(Expression::add(*lhs, *rhs).with_span(span)),
                    Op::Subtract => Ok(Expression::subtract(*lhs, *rhs).with_span(span)),
                    Op::Interval => Ok(Expression::interval(*lhs, *rhs).with_span(span)),
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
