use crate::error::SiffraExecutionError;
use crate::representations::{Angle, Dimension, Float, Quantity, Unit, Value};
use crate::siffra_error;

pub(super) fn bounded_integer(value: &Float, min: i32, max: i32) -> Option<i32> {
    if !value.is_integer() {
        return None;
    }
    if value == &Float::from(0) {
        return (min <= 0 && max >= 0).then_some(0);
    }
    let parsed = value.to_string().replace(',', "").parse::<i32>().ok()?;
    (min..=max).contains(&parsed).then_some(parsed)
}

pub(super) fn radians_dimension() -> Dimension {
    Dimension::new(vec![(
        Unit::Base(Quantity::Angle(Angle::Radian)),
        Float::from(1),
    )])
}

// A temporal point's unit is the zone it is read in, which says nothing useful
// about why an operation was refused.
pub(super) fn unit_label(dimension: &Dimension) -> String {
    if let Some(point) = dimension.as_temporal_point() {
        return if point.is_clock() {
            "a time of day".to_string()
        } else {
            "a date".to_string()
        };
    }
    let label = dimension.to_string();
    if label.is_empty() {
        "unitless".to_string()
    } else {
        label
    }
}

pub(super) fn value_unit(value: &Value) -> String {
    unit_label(&value.dimension())
}

// Unitless angles are radians; physical angles are converted explicitly.
pub(super) fn trig_radians(
    value: &Value,
    function: &str,
    span: Option<crate::parser::Span>,
) -> Result<Float, SiffraExecutionError> {
    if value.dimension().is_unitless() {
        return Ok(value.value());
    }
    value
        .convert(&radians_dimension())
        .map(|converted| converted.value())
        .ok_or_else(|| {
            siffra_error!(
                "Unit Error",
                format!(
                    "{function} requires an angle or unitless input; got {}",
                    value_unit(value)
                ),
                span
            )
        })
}

pub(super) fn require_unitless(
    value: &Value,
    function: &str,
    span: Option<crate::parser::Span>,
) -> Result<Float, SiffraExecutionError> {
    if value.dimension().is_unitless() {
        Ok(value.value())
    } else {
        Err(siffra_error!(
            "Unit Error",
            format!("{function} must be unitless; got {}", value_unit(value)),
            span
        ))
    }
}

pub(super) fn radian_value(value: Float) -> Value {
    Value::new(value, Some(radians_dimension()))
}

// Remove precision noise at exact trig landmarks without erasing tiny inputs.
pub(super) fn round_trig(value: Float, radians: &Float) -> Float {
    let epsilon = Float::parse("1E-50").expect("valid epsilon");
    if value.abs() < epsilon && radians.abs() >= epsilon {
        return Float::from(0);
    }
    if (&value - &Float::from(1)).abs() < epsilon {
        return Float::from(1);
    }
    if (&value + &Float::from(1)).abs() < epsilon {
        return Float::from(-1);
    }
    value.round_inexact()
}

pub(super) fn values_in_first_unit(
    args: &[Value],
    function: &str,
    span: Option<crate::parser::Span>,
) -> Result<(Dimension, Vec<Float>), SiffraExecutionError> {
    let target = args[0].dimension();
    let mut values = Vec::with_capacity(args.len());
    values.push(args[0].value());
    for value in args.iter().skip(1) {
        let converted = value.convert(&target).ok_or_else(|| {
            siffra_error!(
                "Unit Error",
                format!(
                    "{function} cannot mix {} and {}",
                    unit_label(&target),
                    value_unit(value)
                ),
                span
            )
        })?;
        values.push(converted.value());
    }
    Ok((target, values))
}

pub(super) fn decimal_rate(
    value: &Value,
    label: &str,
    span: Option<crate::parser::Span>,
) -> Result<(Float, bool), SiffraExecutionError> {
    if value.dimension().is_percent() {
        return Ok((value.convert(&Dimension::default()).unwrap().value(), true));
    }
    if value.dimension().is_unitless() {
        return Ok((value.value(), false));
    }
    Err(siffra_error!(
        "Unit Error",
        format!("{label} must be % or unitless; got {}", value_unit(value)),
        span
    ))
}

pub(super) fn rate_result(value: Float, as_percent: bool) -> Value {
    let value = Value::new(value, None);
    if as_percent {
        value.convert(&Dimension::percent()).unwrap()
    } else {
        value
    }
}

pub(super) fn modulo(lhs: &Float, rhs: &Float) -> Option<Float> {
    if rhs == &Float::from(0) {
        return None;
    }
    let quotient = (lhs / rhs).floor();
    Some(lhs - &(&quotient * rhs))
}

pub(super) fn gcd_float(lhs: &Float, rhs: &Float) -> Float {
    let mut a = lhs.abs();
    let mut b = rhs.abs();
    while b != Float::from(0) {
        let remainder = modulo(&a, &b).expect("nonzero divisor");
        a = b;
        b = remainder;
    }
    a
}
