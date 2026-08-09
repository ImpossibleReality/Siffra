mod state;

use crate::error::SiffraExecutionError;
pub use state::SiffraState;
use std::ops::Mul;

use crate::evaluation::state::VariableAccessError;
use crate::parser::parse_line;
use crate::parser::representation::ParsedLine;
use crate::representations::{
    Amount, Angle, DerivedUnit, Dimension, Expression, Float, InnerExpression, Length,
    PercentageOperator, Quantity, TemperatureInterval, Time, Unit, Value,
};
use crate::{siffra_error, siffra_try};

pub type EvaluationResult = Result<Option<Value>, SiffraExecutionError>;

const MIN_FACTORIAL_INPUT: i32 = -1000;
const MAX_FACTORIAL_INPUT: i32 = 1000;
const MIN_ROUND_PLACES: i32 = -100;
const MAX_ROUND_PLACES: i32 = 100;
const MAX_COMPOUNDS_PER_YEAR: i32 = 1_000_000;
const MAX_INTEGER_FUNCTION_INPUT: i32 = 1_000_000;
const MAX_COMBINATORIC_INPUT: i32 = 1_000;

fn bounded_integer(value: &Float, min: i32, max: i32) -> Option<i32> {
    if !value.is_integer() {
        return None;
    }
    if value == &Float::from(0) {
        return (min <= 0 && max >= 0).then_some(0);
    }
    let parsed = value.to_string().replace(',', "").parse::<i32>().ok()?;
    (min..=max).contains(&parsed).then_some(parsed)
}

fn radians_dimension() -> Dimension {
    Dimension::new(vec![(
        Unit::Base(Quantity::Angle(Angle::Radian)),
        Float::from(1),
    )])
}

fn unit_label(dimension: &Dimension) -> String {
    let label = dimension.to_string();
    if label.is_empty() {
        "unitless".to_string()
    } else {
        label
    }
}

fn value_unit(value: &Value) -> String {
    unit_label(&value.dimension())
}

// Unitless angles are radians; physical angles are converted explicitly.
fn trig_radians(
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

fn require_unitless(
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

fn radian_value(value: Float) -> Value {
    Value::new(value, Some(radians_dimension()))
}

// Remove precision noise at exact trig landmarks without erasing tiny inputs.
fn round_trig(value: Float, radians: &Float) -> Float {
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

fn values_in_first_unit(
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

fn decimal_rate(
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

fn positive_periods(
    value: &Value,
    label: &str,
    error_title: &str,
    span: Option<crate::parser::Span>,
) -> Result<i32, SiffraExecutionError> {
    if !value.dimension().is_unitless() {
        return Err(siffra_error!(
            "Unit Error",
            format!("{label} must be unitless; got {}", value_unit(value)),
            span
        ));
    }
    let raw = value.value();
    bounded_integer(&raw, 1, MAX_COMPOUNDS_PER_YEAR).ok_or_else(|| {
        siffra_error!(
            error_title,
            format!("{label} must be an integer from 1 through {MAX_COMPOUNDS_PER_YEAR}"),
            span
        )
    })
}

fn rate_result(value: Float, as_percent: bool) -> Value {
    let value = Value::new(value, None);
    if as_percent {
        value.convert(&Dimension::percent()).unwrap()
    } else {
        value
    }
}

fn modulo(lhs: &Float, rhs: &Float) -> Option<Float> {
    if rhs == &Float::from(0) {
        return None;
    }
    let quotient = (lhs / rhs).floor();
    Some(lhs - &(&quotient * rhs))
}

fn gcd_float(lhs: &Float, rhs: &Float) -> Float {
    let mut a = lhs.abs();
    let mut b = rhs.abs();
    while b != Float::from(0) {
        let remainder = modulo(&a, &b).expect("nonzero divisor");
        a = b;
        b = remainder;
    }
    a
}

fn physical_constant(value: &str, terms: Vec<(Unit, i32)>) -> Value {
    Value::new(
        Float::parse(value).expect("valid physical constant"),
        Some(Dimension::new(
            terms
                .into_iter()
                .map(|(unit, power)| (unit, Float::from(power)))
                .collect(),
        )),
    )
}

fn factorial_exact(x: &Float) -> Float {
    let one = Float::from(1.0);
    let mut result = one.clone();
    let mut i = one.clone();
    while matches!(
        i.partial_cmp(x),
        Some(std::cmp::Ordering::Less) | Some(std::cmp::Ordering::Equal)
    ) {
        result = &result * &i;
        i = &i + &one;
    }
    result
}

fn percentage_as_scalar(
    value: &Value,
    span: Option<crate::parser::Span>,
) -> Result<Value, SiffraExecutionError> {
    if !value.dimension().is_percent() {
        return Err(siffra_error!(
            "Percentage Error",
            format!("Expected a percentage; got {}", value_unit(value)),
            span
        ));
    }
    value.convert(&Dimension::default()).ok_or_else(|| {
        siffra_error!(
            "Percentage Error",
            "Unable to convert the percentage to a scalar",
            span
        )
    })
}

fn relative_percentage(
    numerator: Value,
    denominator: Value,
    span: Option<crate::parser::Span>,
) -> Result<Value, SiffraExecutionError> {
    if denominator.value() == Float::from(0) {
        return Err(siffra_error!(
            "Percentage Error",
            "A percentage relative to zero is undefined",
            span
        ));
    }
    let ratio = numerator.try_div(&denominator).ok_or_else(|| {
        siffra_error!(
            "Unit Error",
            format!(
                "Cannot compare {} and {}",
                value_unit(&numerator),
                value_unit(&denominator)
            ),
            span
        )
    })?;
    ratio.convert(&Dimension::percent()).ok_or_else(|| {
        siffra_error!(
            "Unit Error",
            format!(
                "Cannot compare {} and {}",
                value_unit(&numerator),
                value_unit(&denominator)
            ),
            span
        )
    })
}

pub fn evaluate_line(line: &str, state: &mut SiffraState) -> EvaluationResult {
    let parsed_line = parse_line(line)?;
    match parsed_line {
        ParsedLine::Comment | ParsedLine::Header(_) => {
            state.clear_block_total();
            Ok(None)
        }
        ParsedLine::Expression(expr) => {
            let value = match evaluate_expr(&expr.try_into()?, state) {
                Ok(value) => value,
                Err(err) => {
                    state.clear_previous_value();
                    return Err(err);
                }
            };
            state.set_previous_value(value.clone());
            state.add_to_block_total(&value);
            Ok(Some(value))
        }
        ParsedLine::Variable(name, expr) => {
            let value = evaluate_expr(&expr.try_into()?, state);
            match value {
                Ok(value) => {
                    state.set_variable(&name, value.clone());
                    state.set_previous_value(value.clone());
                    state.add_to_block_total(&value);
                    Ok(Some(value))
                }
                Err(err) => {
                    state.error_variable(&name, err.clone());
                    state.clear_previous_value();
                    Err(err)
                }
            }
        }
    }
}

pub fn evaluate_expr(
    expr: &Expression,
    state: &SiffraState,
) -> Result<Value, SiffraExecutionError> {
    match expr.inner() {
        InnerExpression::Constant(name) => Ok(name.clone()),
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
        InnerExpression::Variable(name) => {
            if let Ok(value) = state.get_variable(name) {
                Ok(value.clone())
            } else if let Err(VariableAccessError::Error(source)) = state.get_variable(name) {
                let cause = source.description().unwrap_or_else(|| source.message());
                Err(siffra_error!(
                    "Variable Error",
                    format!("Variable '{name}' is invalid: {cause}"),
                    expr.span()
                ))
            } else {
                match name.as_str() {
                    "pi" => Ok(Value::new(Float::pi(), None)),
                    "e" => Ok(Value::new(Float::e(), None)),
                    "tau" => Ok(Value::new(
                        Float::pi().mul(&Float::parse("2").unwrap()),
                        None,
                    )),
                    "phi" => Ok(Value::new(
                        &(&Float::from(1) + &Float::from(5).sqrt()) / &Float::from(2),
                        None,
                    )),
                    "lightspeed" => Ok(physical_constant(
                        "299792458",
                        vec![
                            (Unit::Base(Quantity::Length(Length::Meter)), 1),
                            (Unit::Base(Quantity::Time(Time::Second)), -1),
                        ],
                    )),
                    "standardgravity" => Ok(physical_constant(
                        "9.80665",
                        vec![
                            (Unit::Base(Quantity::Length(Length::Meter)), 1),
                            (Unit::Base(Quantity::Time(Time::Second)), -2),
                        ],
                    )),
                    "avogadro" => Ok(physical_constant(
                        "6.02214076E23",
                        vec![(Unit::Base(Quantity::Amount(Amount::Mole, None)), -1)],
                    )),
                    "planck" => Ok(physical_constant(
                        "6.62607015E-34",
                        vec![
                            (Unit::Derived(DerivedUnit::Joule), 1),
                            (Unit::Base(Quantity::Time(Time::Second)), 1),
                        ],
                    )),
                    "boltzmann" => Ok(physical_constant(
                        "1.380649E-23",
                        vec![
                            (Unit::Derived(DerivedUnit::Joule), 1),
                            (
                                Unit::Base(Quantity::Temperature(TemperatureInterval::Kelvin)),
                                -1,
                            ),
                        ],
                    )),
                    "elementarycharge" => Ok(physical_constant(
                        "1.602176634E-19",
                        vec![(Unit::Derived(DerivedUnit::Coulomb), 1)],
                    )),
                    "prev" | "ans" | "previous" | "that" => {
                        if let Some(prev) = state.previous_value() {
                            Ok(prev.clone())
                        } else {
                            Err(siffra_error!(
                                "Name Error",
                                "No previous value to use",
                                expr.span()
                            ))
                        }
                    }
                    "total" | "sum" => {
                        if let Some(total) = state.block_total() {
                            Ok(total.clone())
                        } else {
                            let description = state.block_total_error().map_or_else(
                                || "Block total is unavailable".to_string(),
                                |(total, value)| {
                                    format!(
                                        "Block total cannot mix {} and {}",
                                        unit_label(total),
                                        unit_label(value)
                                    )
                                },
                            );
                            Err(siffra_error!("Total Error", description, expr.span()))
                        }
                    }
                    "k" | "K" => Ok(Value::new(Float::parse("1000").unwrap(), None)),
                    _ => Err(siffra_error!(
                        "Name Error",
                        format!("Variable '{}' not found", name),
                        expr.span()
                    )),
                }
            }
        }
        InnerExpression::FunctionCall { name, args } => {
            let args = args
                .iter()
                .map(|arg| evaluate_expr(arg, state))
                .collect::<Result<Vec<_>, _>>()?;
            match &**name {
                "round" => match args.len() {
                    1 => Ok(Value::new(
                        args[0].value().round_decimal_places(0),
                        Some(args[0].dimension()),
                    )),
                    2 => {
                        if !args[1].dimension().is_unitless() {
                            return Err(siffra_error!(
                                "Unit Error",
                                format!(
                                    "Decimal places must be unitless; got {}",
                                    value_unit(&args[1])
                                ),
                                expr.span()
                            ));
                        }
                        let places = bounded_integer(
                            &args[1].value(),
                            MIN_ROUND_PLACES,
                            MAX_ROUND_PLACES,
                        )
                        .ok_or_else(|| {
                            siffra_error!(
                                "Round Error",
                                format!(
                                    "Decimal places must be an integer from {MIN_ROUND_PLACES} through {MAX_ROUND_PLACES}"
                                ),
                                expr.span()
                            )
                        })?;
                        Ok(Value::new(
                            args[0].value().round_decimal_places(places),
                            Some(args[0].dimension()),
                        ))
                    }
                    _ => Err(siffra_error!(
                        "Argument Error",
                        "round takes 1 or 2 arguments",
                        expr.span()
                    )),
                },
                "floor" => {
                    if args.len() == 1 {
                        Ok(Value::new(
                            args[0].value().floor(),
                            Some(args[0].dimension()),
                        ))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "floor takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "ceil" => {
                    if args.len() == 1 {
                        Ok(Value::new(
                            args[0].value().ceil(),
                            Some(args[0].dimension()),
                        ))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "ceil takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "trunc" => {
                    if args.len() == 1 {
                        Ok(Value::new(
                            args[0].value().trunc(),
                            Some(args[0].dimension()),
                        ))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "trunc takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "sign" => {
                    if args.len() != 1 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "sign takes exactly 1 argument",
                            expr.span()
                        ));
                    }
                    let zero = Float::from(0);
                    let sign = if args[0].value() < zero {
                        -1
                    } else if args[0].value() > zero {
                        1
                    } else {
                        0
                    };
                    Ok(Value::new(Float::from(sign), None))
                }
                "mod" => {
                    if args.len() != 2 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "mod takes exactly 2 arguments",
                            expr.span()
                        ));
                    }
                    let (dimension, values) = values_in_first_unit(&args, "mod", expr.span())?;
                    let remainder = modulo(&values[0], &values[1]).ok_or_else(|| {
                        siffra_error!("Division Error", "The modulus cannot be zero", expr.span())
                    })?;
                    Ok(Value::new(remainder, Some(dimension)))
                }
                "min" | "max" => {
                    if args.is_empty() {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} takes at least 1 argument"),
                            expr.span()
                        ));
                    }
                    let (dimension, values) = values_in_first_unit(&args, name, expr.span())?;
                    let mut selected = values[0].clone();
                    for value in values.iter().skip(1) {
                        let replace = if name == "min" {
                            value < &selected
                        } else {
                            value > &selected
                        };
                        if replace {
                            selected = value.clone();
                        }
                    }
                    Ok(Value::new(selected, Some(dimension)))
                }
                "clamp" => {
                    if args.len() != 3 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "clamp takes exactly 3 arguments",
                            expr.span()
                        ));
                    }
                    let (dimension, values) = values_in_first_unit(&args, "clamp", expr.span())?;
                    let value = &values[0];
                    let low = &values[1];
                    let high = &values[2];
                    if low > high {
                        return Err(siffra_error!(
                            "Clamp Error",
                            "The lower bound cannot be greater than the upper bound",
                            expr.span()
                        ));
                    }
                    let clamped = if value < low {
                        low.clone()
                    } else if value > high {
                        high.clone()
                    } else {
                        value.clone()
                    };
                    Ok(Value::new(clamped, Some(dimension)))
                }
                "mean" => {
                    if args.is_empty() {
                        return Err(siffra_error!(
                            "Argument Error",
                            "mean takes at least 1 argument",
                            expr.span()
                        ));
                    }
                    let (dimension, values) = values_in_first_unit(&args, "mean", expr.span())?;
                    let mut total = Float::from(0);
                    for value in &values {
                        total = &total + value;
                    }
                    Ok(Value::new(
                        &total / &Float::from(values.len() as i64),
                        Some(dimension),
                    ))
                }
                "gcd" | "lcm" => {
                    if args.len() != 2 {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} takes exactly 2 arguments"),
                            expr.span()
                        ));
                    }
                    if let Some((index, arg)) = args
                        .iter()
                        .enumerate()
                        .find(|(_, arg)| !arg.dimension().is_unitless())
                    {
                        return Err(siffra_error!(
                            "Unit Error",
                            format!(
                                "{name} argument {} must be unitless; got {}",
                                index + 1,
                                value_unit(arg)
                            ),
                            expr.span()
                        ));
                    }
                    let integers = args
                        .iter()
                        .map(|arg| {
                            bounded_integer(
                                &arg.value(),
                                -MAX_INTEGER_FUNCTION_INPUT,
                                MAX_INTEGER_FUNCTION_INPUT,
                            )
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or_else(|| {
                            siffra_error!(
                                "Integer Error",
                                format!(
                                    "{name} arguments must be integers from -{MAX_INTEGER_FUNCTION_INPUT} through {MAX_INTEGER_FUNCTION_INPUT}"
                                ),
                                expr.span()
                            )
                        })?;
                    let lhs = Float::from(integers[0]);
                    let rhs = Float::from(integers[1]);
                    let gcd = gcd_float(&lhs, &rhs);
                    let result = if name == "gcd" || gcd == Float::from(0) {
                        gcd
                    } else {
                        (&(&lhs / &gcd) * &rhs).abs()
                    };
                    Ok(Value::new(result, None))
                }
                "choose" | "permute" => {
                    if args.len() != 2 {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} takes exactly 2 arguments"),
                            expr.span()
                        ));
                    }
                    if let Some((index, arg)) = args
                        .iter()
                        .enumerate()
                        .find(|(_, arg)| !arg.dimension().is_unitless())
                    {
                        return Err(siffra_error!(
                            "Unit Error",
                            format!(
                                "{name} argument {} must be unitless; got {}",
                                index + 1,
                                value_unit(arg)
                            ),
                            expr.span()
                        ));
                    }
                    let n = bounded_integer(&args[0].value(), 0, MAX_COMBINATORIC_INPUT);
                    let k = bounded_integer(&args[1].value(), 0, MAX_COMBINATORIC_INPUT);
                    let (Some(n), Some(k)) = (n, k) else {
                        return Err(siffra_error!(
                            "Integer Error",
                            format!(
                                "{name} arguments must be non-negative integers no greater than {MAX_COMBINATORIC_INPUT}"
                            ),
                            expr.span()
                        ));
                    };
                    if k > n {
                        return Err(siffra_error!(
                            "Domain Error",
                            "k cannot be greater than n",
                            expr.span()
                        ));
                    }
                    let mut result = Float::from(1);
                    if name == "choose" {
                        let k = k.min(n - k);
                        for i in 1..=k {
                            result = &result * &Float::from(n - k + i);
                            result = &result / &Float::from(i);
                        }
                    } else {
                        for i in 0..k {
                            result = &result * &Float::from(n - i);
                        }
                    }
                    Ok(Value::new(result, None))
                }
                "atan2" => {
                    if args.len() != 2 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "atan2 takes exactly 2 arguments",
                            expr.span()
                        ));
                    }
                    let (_, values) = values_in_first_unit(&args, "atan2", expr.span())?;
                    let angle = values[0].atan2(&values[1]).ok_or_else(|| {
                        siffra_error!(
                            "Domain Error",
                            "atan2 is undefined when both arguments are zero",
                            expr.span()
                        )
                    })?;
                    Ok(radian_value(angle.round_inexact()))
                }
                "hypot" => {
                    if args.len() != 2 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "hypot takes exactly 2 arguments",
                            expr.span()
                        ));
                    }
                    let (dimension, values) = values_in_first_unit(&args, "hypot", expr.span())?;
                    let squared = &(&values[0] * &values[0]) + &(&values[1] * &values[1]);
                    Ok(Value::new(squared.sqrt().round_inexact(), Some(dimension)))
                }
                "apy" | "ear" => {
                    if args.len() != 2 {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} takes exactly 2 arguments"),
                            expr.span()
                        ));
                    }
                    let (rate, rate_was_percent) = decimal_rate(&args[0], "APR", expr.span())?;
                    let compounds =
                        positive_periods(&args[1], "Compounds per year", "APY Error", expr.span())?;
                    let n = Float::from(compounds);
                    let one = Float::from(1);
                    let periodic_base = &one + &(&rate / &n);
                    if periodic_base <= Float::from(0) {
                        return Err(siffra_error!(
                            "APY Error",
                            "The APR is too negative for this compounding frequency",
                            expr.span()
                        ));
                    }
                    let growth = periodic_base.pow(&n);
                    let annual_yield = (&growth - &one).round_inexact();
                    Ok(rate_result(annual_yield, rate_was_percent))
                }
                "apr" => {
                    if args.len() != 2 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "apr takes exactly 2 arguments",
                            expr.span()
                        ));
                    }
                    let (yield_rate, yield_was_percent) =
                        decimal_rate(&args[0], "APY", expr.span())?;
                    let compounds =
                        positive_periods(&args[1], "Compounds per year", "APR Error", expr.span())?;
                    let n = Float::from(compounds);
                    let one = Float::from(1);
                    let annual_base = &one + &yield_rate;
                    if annual_base <= Float::from(0) {
                        return Err(siffra_error!(
                            "APR Error",
                            "APY must be greater than -100%",
                            expr.span()
                        ));
                    }
                    let root = annual_base.pow(&(&one / &n));
                    let nominal_rate = (&n * &(&root - &one)).round_inexact();
                    Ok(rate_result(nominal_rate, yield_was_percent))
                }
                "fv" | "compound" | "pv" | "pmt" => {
                    if args.len() != 3 {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} takes exactly 3 arguments"),
                            expr.span()
                        ));
                    }
                    let (rate, _) = decimal_rate(&args[1], "rate", expr.span())?;
                    let periods =
                        positive_periods(&args[2], "Periods", "Finance Error", expr.span())?;
                    let one = Float::from(1);
                    let n = Float::from(periods);
                    let base = &one + &rate;
                    if base <= Float::from(0) {
                        return Err(siffra_error!(
                            "Finance Error",
                            "The per-period rate must be greater than -100%",
                            expr.span()
                        ));
                    }
                    let result = match name.as_str() {
                        "fv" | "compound" => &args[0].value() * &base.pow(&n),
                        "pv" => {
                            let growth = base.pow(&n);
                            if growth == Float::from(0) {
                                return Err(siffra_error!(
                                    "Finance Error",
                                    "Present value would require division by zero",
                                    expr.span()
                                ));
                            }
                            &args[0].value() / &growth
                        }
                        "pmt" if rate == Float::from(0) => &args[0].value() / &n,
                        "pmt" => {
                            let discount = base.pow(&Float::from(-periods)).round_inexact();
                            let denominator = &one - &discount;
                            if denominator == Float::from(0) {
                                return Err(siffra_error!(
                                    "Finance Error",
                                    "Payment cannot be calculated for this rate and period count",
                                    expr.span()
                                ));
                            }
                            &(&args[0].value() * &rate) / &denominator
                        }
                        _ => unreachable!(),
                    };
                    Ok(Value::new(
                        result.round_inexact(),
                        Some(args[0].dimension()),
                    ))
                }
                "gamma" => {
                    if args.len() != 1 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "gamma takes exactly 1 argument",
                            expr.span()
                        ));
                    }
                    if !args[0].dimension().is_unitless() {
                        return Err(siffra_error!(
                            "Unit Error",
                            format!("Gamma must be unitless; got {}", value_unit(&args[0])),
                            expr.span()
                        ));
                    }
                    let x = args[0].value();
                    if x < Float::from(MIN_FACTORIAL_INPUT) || x > Float::from(MAX_FACTORIAL_INPUT)
                    {
                        return Err(siffra_error!(
                            "Gamma Error",
                            format!(
                                "Gamma is supported for inputs from {MIN_FACTORIAL_INPUT} through {MAX_FACTORIAL_INPUT}"
                            ),
                            expr.span()
                        ));
                    }
                    if x <= Float::from(0) && x.is_integer() {
                        return Err(siffra_error!(
                            "Gamma Error",
                            "Gamma is undefined for zero and negative integers",
                            expr.span()
                        ));
                    }
                    let result = if x.is_nonneg_integer() {
                        factorial_exact(&(&x - &Float::from(1)))
                    } else {
                        x.gamma().round_significant_digits(14)
                    };
                    Ok(Value::new(result, None))
                }
                "factorial" => {
                    if args.len() == 1 {
                        if !args[0].dimension().is_unitless() {
                            return Err(siffra_error!(
                                "Unit Error",
                                format!("Factorial must be unitless; got {}", value_unit(&args[0])),
                                expr.span()
                            ));
                        }
                        let x = args[0].value.clone();
                        let min = Float::from(MIN_FACTORIAL_INPUT);
                        let max = Float::from(MAX_FACTORIAL_INPUT);
                        if x < min || x > max {
                            return Err(siffra_error!(
                                "Factorial Error",
                                format!(
                                    "Factorial is supported for inputs from {} through {}",
                                    MIN_FACTORIAL_INPUT, MAX_FACTORIAL_INPUT
                                ),
                                expr.span()
                            ));
                        }
                        if x < Float::from(0) && x.is_integer() {
                            return Err(siffra_error!(
                                "Factorial Error",
                                "Factorial is undefined for negative integers",
                                expr.span()
                            ));
                        }
                        let result = if x.is_nonneg_integer() {
                            factorial_exact(&x)
                        } else {
                            (&x + &Float::from(1.0))
                                .gamma()
                                .round_significant_digits(14)
                        };
                        Ok(Value::new(result, None))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "factorial takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "root" => match args.len() {
                    1 => {
                        if args[0].value() < Float::from(0) {
                            return Err(siffra_error!(
                                "Domain Error",
                                "A square root requires a non-negative value",
                                expr.span()
                            ));
                        }
                        Ok(siffra_try!(
                            args[0]
                                .try_pow(&Value::new(Float::from(0.5), None))
                                .ok_or(()),
                            "Root Error",
                            "Error taking root",
                            expr.span()
                        ))
                    }
                    2 => {
                        let degree = require_unitless(&args[0], "The root degree", expr.span())?;
                        if degree == Float::from(0) {
                            return Err(siffra_error!(
                                "Domain Error",
                                "The root degree cannot be zero",
                                expr.span()
                            ));
                        }
                        let exponent = &Float::from(1) / &degree;
                        let exponent_value = Value::new(exponent, None);
                        if args[1].value() < Float::from(0) {
                            let odd_integer_degree = degree.is_integer()
                                && modulo(&degree.abs(), &Float::from(2)) == Some(Float::from(1));
                            if !odd_integer_degree {
                                return Err(siffra_error!(
                                    "Domain Error",
                                    "A negative value requires an odd integer root degree",
                                    expr.span()
                                ));
                            }
                            let positive =
                                Value::new(args[1].value().abs(), Some(args[1].dimension()));
                            return Ok(siffra_try!(
                                positive.try_pow(&exponent_value).ok_or(()),
                                "Root Error",
                                "Error taking root",
                                expr.span()
                            )
                            .negate());
                        }
                        Ok(siffra_try!(
                            args[1].try_pow(&exponent_value).ok_or(()),
                            "Root Error",
                            "Error taking root",
                            expr.span()
                        ))
                    }
                    _ => Err(siffra_error!(
                        "Argument Error",
                        "root takes 1 or 2 arguments",
                        expr.span()
                    )),
                },
                "log" => {
                    if args.len() == 1 {
                        let value = require_unitless(&args[0], "log", expr.span())?;
                        if value <= Float::from(0) {
                            return Err(siffra_error!(
                                "Domain Error",
                                "log requires a positive number",
                                expr.span()
                            ));
                        }
                        Ok(Value::new(value.log10().round_inexact(), None))
                    } else if args.len() == 2 {
                        let base = require_unitless(&args[0], "log", expr.span())?;
                        let value = require_unitless(&args[1], "log", expr.span())?;
                        if base <= Float::from(0)
                            || base == Float::from(1)
                            || value <= Float::from(0)
                        {
                            return Err(siffra_error!(
                                "Domain Error",
                                "The logarithm base must be positive and not 1, and its argument must be positive",
                                expr.span()
                            ));
                        }
                        Ok(Value::new((&value.ln() / &base.ln()).round_inexact(), None))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "log function takes either 1 or 2 arguments",
                            expr.span()
                        ))
                    }
                }
                "ln" => {
                    if args.len() == 1 {
                        let value = require_unitless(&args[0], "ln", expr.span())?;
                        if value <= Float::from(0) {
                            return Err(siffra_error!(
                                "Domain Error",
                                "ln requires a positive number",
                                expr.span()
                            ));
                        }
                        Ok(Value::new(value.ln().round_inexact(), None))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "ln function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "sqrt" => {
                    if args.len() == 1 {
                        if args[0].value() < Float::from(0) {
                            return Err(siffra_error!(
                                "Domain Error",
                                "sqrt requires a non-negative value",
                                expr.span()
                            ));
                        }
                        Ok(siffra_try!(
                            args[0]
                                .try_pow(&Value::new(Float::from(0.5), None))
                                .ok_or(()),
                            "Root Error",
                            "Error taking square root",
                            expr.span()
                        ))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "sqrt function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "sin" | "cos" | "tan" => {
                    if args.len() != 1 {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} function takes exactly 1 argument"),
                            expr.span()
                        ));
                    }
                    let radians = trig_radians(&args[0], name, expr.span())?;
                    let value = match name.as_str() {
                        "sin" => radians.sin(),
                        "cos" => radians.cos(),
                        "tan" => radians.tan(),
                        _ => unreachable!(),
                    };
                    Ok(Value::new(round_trig(value, &radians), None))
                }
                "sinh" | "cosh" | "tanh" => {
                    if args.len() != 1 {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} takes exactly 1 argument"),
                            expr.span()
                        ));
                    }
                    if !args[0].dimension().is_unitless() {
                        return Err(siffra_error!(
                            "Unit Error",
                            format!("{name} must be unitless; got {}", value_unit(&args[0])),
                            expr.span()
                        ));
                    }
                    let value = match name.as_str() {
                        "sinh" => args[0].value().sinh(),
                        "cosh" => args[0].value().cosh(),
                        "tanh" => args[0].value().tanh(),
                        _ => unreachable!(),
                    };
                    Ok(Value::new(value.round_inexact(), None))
                }
                "asin" | "acos" | "atan" => {
                    if args.len() != 1 {
                        return Err(siffra_error!(
                            "Argument Error",
                            format!("{name} function takes exactly 1 argument"),
                            expr.span()
                        ));
                    }
                    let input = require_unitless(&args[0], name, expr.span())?;
                    if matches!(name.as_str(), "asin" | "acos")
                        && (input < Float::from(-1) || input > Float::from(1))
                    {
                        return Err(siffra_error!(
                            "Domain Error",
                            format!("{name} requires a value from -1 through 1"),
                            expr.span()
                        ));
                    }
                    let value = match name.as_str() {
                        "asin" => input.asin(),
                        "acos" => input.acos(),
                        "atan" => input.atan(),
                        _ => unreachable!(),
                    };
                    Ok(radian_value(value.round_inexact()))
                }
                "exp" => {
                    if args.len() == 1 {
                        let value = require_unitless(&args[0], "exp", expr.span())?;
                        Ok(Value::new(value.exp().round_inexact(), None))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "exp function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }
                "abs" => {
                    if args.len() == 1 {
                        Ok(Value::new(
                            args[0].value.clone().abs(),
                            Some(args[0].dimension()),
                        ))
                    } else {
                        Err(siffra_error!(
                            "Argument Error",
                            "abs function takes exactly 1 argument",
                            expr.span()
                        ))
                    }
                }

                "simplify" => {
                    if args.len() != 1 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "simplify takes exactly 1 argument",
                            expr.span()
                        ));
                    }
                    match args[0].dimension().collapse_to_named() {
                        Some(named) => {
                            Ok(args[0].convert(&named).unwrap_or_else(|| args[0].clone()))
                        }
                        None => Ok(args[0].clone()),
                    }
                }
                "expand" => {
                    if args.len() != 1 {
                        return Err(siffra_error!(
                            "Argument Error",
                            "expand takes exactly 1 argument",
                            expr.span()
                        ));
                    }
                    let expanded = args[0].dimension().expand();
                    Ok(args[0]
                        .convert(&expanded)
                        .unwrap_or_else(|| args[0].clone()))
                }

                name => Err(siffra_error!(
                    "Name Error",
                    format!("The function '{}' is not defined", name),
                    expr.function_name_span().or(expr.span())
                )),
            }
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

            match operator {
                PercentageOperator::On => {
                    percentage_as_scalar(&lhs, expr.span())?;
                    rhs.try_add(&lhs).ok_or_else(|| {
                        siffra_error!(
                            "Percentage Error",
                            "Unable to add this percentage to the value",
                            expr.span()
                        )
                    })
                }
                PercentageOperator::Off => {
                    percentage_as_scalar(&lhs, expr.span())?;
                    rhs.try_sub(&lhs).ok_or_else(|| {
                        siffra_error!(
                            "Percentage Error",
                            "Unable to subtract this percentage from the value",
                            expr.span()
                        )
                    })
                }
                PercentageOperator::RelativeOf => relative_percentage(lhs, rhs, expr.span()),
                PercentageOperator::RelativeOn => {
                    let increase = lhs.try_sub(&rhs).ok_or_else(|| {
                        siffra_error!(
                            "Unit Error",
                            format!(
                                "Cannot compare {} and {}",
                                value_unit(&lhs),
                                value_unit(&rhs)
                            ),
                            expr.span()
                        )
                    })?;
                    relative_percentage(increase, rhs, expr.span())
                }
                PercentageOperator::RelativeOff => {
                    let decrease = rhs.try_sub(&lhs).ok_or_else(|| {
                        siffra_error!(
                            "Unit Error",
                            format!(
                                "Cannot compare {} and {}",
                                value_unit(&lhs),
                                value_unit(&rhs)
                            ),
                            expr.span()
                        )
                    })?;
                    relative_percentage(decrease, rhs, expr.span())
                }
                PercentageOperator::OfWhat
                | PercentageOperator::OnWhat
                | PercentageOperator::OffWhat => {
                    let percentage = percentage_as_scalar(&lhs, expr.span())?;
                    let one = Float::from(1);
                    let divisor = match operator {
                        PercentageOperator::OfWhat => percentage.value(),
                        PercentageOperator::OnWhat => &one + &percentage.value(),
                        PercentageOperator::OffWhat => &one - &percentage.value(),
                        _ => unreachable!(),
                    };
                    if divisor == Float::from(0) {
                        return Err(siffra_error!(
                            "Percentage Error",
                            "This inverse percentage calculation would divide by zero",
                            expr.span()
                        ));
                    }
                    rhs.try_div(&Value::new(divisor, None)).ok_or_else(|| {
                        siffra_error!(
                            "Percentage Error",
                            "Unable to solve this inverse percentage calculation",
                            expr.span()
                        )
                    })
                }
            }
        }
    }
}
