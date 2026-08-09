use super::*;

pub(super) fn evaluate(
    name: &str,
    args: &[Value],
    expr: &Expression,
) -> Result<Value, SiffraExecutionError> {
    match name {
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
                    let positive = Value::new(args[1].value().abs(), Some(args[1].dimension()));
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
                if base <= Float::from(0) || base == Float::from(1) || value <= Float::from(0) {
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
            let value = match name {
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
            let value = match name {
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
            if matches!(name, "asin" | "acos")
                && (input < Float::from(-1) || input > Float::from(1))
            {
                return Err(siffra_error!(
                    "Domain Error",
                    format!("{name} requires a value from -1 through 1"),
                    expr.span()
                ));
            }
            let value = match name {
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

        _ => unreachable!(),
    }
}
