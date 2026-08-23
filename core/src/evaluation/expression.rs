use super::SiffraState;
use super::functions::evaluate_function;
use super::helpers::{unit_label, value_unit};
use super::percentage::evaluate_percentage;
use super::temporal::{resolve_clock, resolve_instant};
use super::variables::resolve_variable;
use crate::error::SiffraExecutionError;
use crate::representations::{Dimension, Expression, Float, InnerExpression, Unit, Value, ladder};
use crate::{siffra_error, siffra_try};

/// Failiure to resolve variable after `to` should reference both units + variables
fn evaluate_endpoint(
    expr: &Expression,
    state: &SiffraState,
) -> Result<Value, SiffraExecutionError> {
    evaluate_expr(expr, state).map_err(|error| match expr.inner() {
        InnerExpression::Variable(name) if error.message() == "Name Error" => siffra_error!(
            "Name Error",
            format!("'{name}' is not a known unit or a defined value"),
            expr.span()
        ),
        _ => error,
    })
}

/// Converts into a mixed-unit target: the value dimension becomes the smallest rung,
/// which is the one the scalar is then read in, and the rungs become a display hint.
fn evaluate_ladder(
    value: Value,
    units: &[Unit],
    span: Option<crate::parser::Span>,
) -> Result<Value, SiffraExecutionError> {
    if let Err(error) = ladder::validate(units) {
        return Err(siffra_error!("Unit Error", error.message(), span));
    }
    let label = || {
        units
            .iter()
            .map(Unit::shorthand)
            .collect::<Vec<_>>()
            .join(",")
    };

    let smallest = ladder::rung_dimension(units.last().expect("a validated ladder has rungs"));
    let converted = siffra_try!(
        value.convert(&smallest).ok_or(()),
        "Unit Error",
        format!("Cannot convert {} to {}", value_unit(&value), label()),
        span
    );

    // Combined ranges are independent totals — 59 days and 1,416 hours can each
    // be right while the ladder output is ambiguous. Anchored ranges are fine, since
    // they have a single total.
    //
    // E.X. `1/1/2026 to 1/31/2026` is a single range, but
    // `1/1/2026 to 1/31/2026 + 1/1/2026 to 2/28/2026` is two ranges that would not
    // make sense to split into a ladder.
    if ladder::has_calendar_rung(units)
        && converted
            .basis()
            .is_some_and(|basis| basis.anchors().is_none())
    {
        return Err(siffra_error!(
            "Range Error",
            format!(
                "Cannot split a combined range into {}; convert to a single unit instead",
                units
                    .iter()
                    .map(Unit::shorthand)
                    .collect::<Vec<_>>()
                    .join(" + ")
            ),
            span
        ));
    }

    Ok(converted.with_ladder(units.to_vec()))
}

/// A mixed calendar duration shifts one rung at a time. This is deliberately
/// evaluated from `Value`s so a literal, a variable and a nested expression
/// all have the same behavior.
fn temporal_ladder_shift(point: &Value, duration: &Value, subtract: bool) -> Option<Option<Value>> {
    if !point.dimension().is_temporal_point() || !duration.dimension().is_duration() {
        return None;
    }
    let units = duration.display().ladder.as_deref()?;
    if !ladder::has_calendar_rung(units) {
        return None;
    }
    let rungs = match ladder::shift_rungs(
        &duration.value(),
        &duration.dimension(),
        duration.basis(),
        units,
    ) {
        Some(rungs) => rungs,
        None => return Some(None),
    };

    Some(
        rungs
            .into_iter()
            .try_fold(point.clone(), |point, (amount, unit)| {
                let rung = Value::new(amount, Some(ladder::rung_dimension(&unit)));
                if subtract {
                    point.try_sub(&rung)
                } else {
                    point.try_add(&rung)
                }
            }),
    )
}

fn add_values(a: &Value, b: &Value) -> Option<Value> {
    if let Some(result) = temporal_ladder_shift(a, b, false) {
        return result;
    }
    if let Some(result) = temporal_ladder_shift(b, a, false) {
        return result;
    }
    a.try_add(b)
}

fn subtract_values(a: &Value, b: &Value) -> Option<Value> {
    temporal_ladder_shift(a, b, true).unwrap_or_else(|| a.try_sub(b))
}

pub fn evaluate_expr(
    expr: &Expression,
    state: &SiffraState,
) -> Result<Value, SiffraExecutionError> {
    match expr.inner() {
        InnerExpression::Constant(value) => Ok((**value).clone()),
        InnerExpression::Instant(literal) => resolve_instant(literal, state, expr.span()),
        InnerExpression::Clock(literal) => resolve_clock(literal, state, expr.span()),
        InnerExpression::Interval { start, end } => {
            let start = evaluate_endpoint(start, state)?;
            let end = evaluate_endpoint(end, state)?;
            if !start.dimension().is_temporal_point() || !end.dimension().is_temporal_point() {
                return Err(siffra_error!(
                    "Range Error",
                    format!(
                        "A range runs between two dates or two times; got {} and {}",
                        value_unit(&start),
                        value_unit(&end)
                    ),
                    expr.span()
                ));
            }
            // Subtracting the endpoints is already what anchors a span to them.
            Ok(siffra_try!(
                end.try_sub(&start).ok_or(()),
                "Range Error",
                "A range needs two dates or two times of day, not one of each",
                expr.span()
            ))
        }
        InnerExpression::Negate(value) => Ok(evaluate_expr(value, state)?.negate()),
        InnerExpression::Percent(value) => {
            let value = evaluate_expr(value, state)?;
            if !value.dimension().is_unitless() {
                return Err(siffra_error!(
                    "Percentage Error",
                    format!("% requires unitless input; got {}", value_unit(&value)),
                    expr.span()
                ));
            }
            Ok(value.with_units(Dimension::percent()))
        }
        InnerExpression::Convert(val, dim) => {
            let val = evaluate_expr(val, state)?;
            if dim.is_unitless() {
                // Scalar conversions consume their factor; physical units only relabel.
                if val.dimension().dimensionality().is_empty() {
                    return Ok(val
                        .convert(&Dimension::default())
                        .unwrap_or_else(|| val.with_units(Dimension::default())));
                }
                Ok(val.with_units(Dimension::default()))
            } else if val.dimension().is_unitless() {
                // Bare values scale into scalar targets and otherwise only gain units.
                if dim.dimensionality().is_empty() {
                    return Ok(val
                        .convert(dim)
                        .unwrap_or_else(|| val.with_units(dim.clone())));
                }
                Ok(val.with_units(dim.clone()))
            } else {
                Ok(siffra_try!(
                    val.convert(dim).ok_or(()),
                    "Unit Error",
                    format!("Cannot convert {} to {}", value_unit(&val), unit_label(dim)),
                    expr.span()
                ))
            }
        }
        InnerExpression::ConvertLadder(value, units) => {
            let value = evaluate_expr(value, state)?;
            evaluate_ladder(value, units, expr.span())
        }
        InnerExpression::Variable(name) => resolve_variable(name, state, expr.span()),
        InnerExpression::FunctionCall { name, args } => {
            let args = args
                .iter()
                .map(|arg| evaluate_expr(arg, state))
                .collect::<Result<Vec<_>, _>>()?;
            evaluate_function(name, &args, expr)
        }
        InnerExpression::Multiply(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                a.try_mul(&b).ok_or(()),
                "Unit Error",
                format!("Cannot multiply {} and {}", value_unit(&a), value_unit(&b)),
                expr.span()
            ))
        }
        InnerExpression::Divide(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                a.try_div(&b).ok_or(()),
                "Division Error",
                "Division by zero",
                expr.span()
            ))
        }
        InnerExpression::Add(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                add_values(&a, &b).ok_or(()),
                "Unit Error",
                format!("Cannot add {} and {}", value_unit(&a), value_unit(&b)),
                expr.span()
            ))
        }
        InnerExpression::Subtract(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                subtract_values(&a, &b).ok_or(()),
                "Unit Error",
                format!("Cannot subtract {} and {}", value_unit(&a), value_unit(&b)),
                expr.span()
            ))
        }
        InnerExpression::Exponent(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            if !b.dimension().is_unitless() {
                return Err(siffra_error!(
                    "Unit Error",
                    format!("Exponent must be unitless; got {}", value_unit(&b)),
                    expr.span()
                ));
            }
            if a.value() == Float::from(0) && b.value() < Float::from(0) {
                return Err(siffra_error!(
                    "Domain Error",
                    "Zero cannot be raised to a negative power",
                    expr.span()
                ));
            }
            let result = siffra_try!(
                a.try_pow(&b).ok_or(()),
                "Unit Error",
                format!("Exponent must be unitless; got {}", value_unit(&b)),
                expr.span()
            );
            if result.value().is_nan() {
                return Err(siffra_error!(
                    "Domain Error",
                    "This power has no real-number result",
                    expr.span()
                ));
            }
            Ok(result)
        }
        InnerExpression::PercentageOperation { lhs, rhs, operator } => {
            let lhs = evaluate_expr(lhs, state)?;
            let rhs = evaluate_expr(rhs, state)?;
            evaluate_percentage(operator, lhs, rhs, expr.span())
        }
    }
}
