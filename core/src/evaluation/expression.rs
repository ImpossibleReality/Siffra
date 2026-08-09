use super::SiffraState;
use super::functions::evaluate_function;
use super::helpers::{unit_label, value_unit};
use super::percentage::evaluate_percentage;
use super::temporal::{resolve_clock, resolve_instant};
use super::variables::resolve_variable;
use crate::error::SiffraExecutionError;
use crate::representations::{Dimension, Expression, Float, InnerExpression, Value};
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

pub fn evaluate_expr(
    expr: &Expression,
    state: &SiffraState,
) -> Result<Value, SiffraExecutionError> {
    match expr.inner() {
        InnerExpression::Constant(name) => Ok(name.clone()),
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
                a.try_add(&b).ok_or(()),
                "Unit Error",
                format!("Cannot add {} and {}", value_unit(&a), value_unit(&b)),
                expr.span()
            ))
        }
        InnerExpression::Subtract(a, b) => {
            let a = evaluate_expr(a, state)?;
            let b = evaluate_expr(b, state)?;
            Ok(siffra_try!(
                a.try_sub(&b).ok_or(()),
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
