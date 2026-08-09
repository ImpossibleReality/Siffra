use super::*;

pub(super) fn evaluate(
    name: &str,
    args: &[Value],
    expr: &Expression,
) -> Result<Value, SiffraExecutionError> {
    match name {
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
            if x < Float::from(MIN_FACTORIAL_INPUT) || x > Float::from(MAX_FACTORIAL_INPUT) {
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
        _ => unreachable!(),
    }
}
