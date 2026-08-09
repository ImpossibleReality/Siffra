use super::*;

pub(super) fn evaluate(
    name: &str,
    args: &[Value],
    expr: &Expression,
) -> Result<Value, SiffraExecutionError> {
    match name {
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
            let (dimension, values) = values_in_first_unit(args, "mod", expr.span())?;
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
            let (dimension, values) = values_in_first_unit(args, name, expr.span())?;
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
            let (dimension, values) = values_in_first_unit(args, "clamp", expr.span())?;
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
            let (dimension, values) = values_in_first_unit(args, "mean", expr.span())?;
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
            let (_, values) = values_in_first_unit(args, "atan2", expr.span())?;
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
            let (dimension, values) = values_in_first_unit(args, "hypot", expr.span())?;
            let squared = &(&values[0] * &values[0]) + &(&values[1] * &values[1]);
            Ok(Value::new(squared.sqrt().round_inexact(), Some(dimension)))
        }
        _ => unreachable!(),
    }
}
