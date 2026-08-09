use super::*;

pub(super) fn evaluate(
    name: &str,
    args: &[Value],
    expr: &Expression,
) -> Result<Value, SiffraExecutionError> {
    match name {
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
            let (yield_rate, yield_was_percent) = decimal_rate(&args[0], "APY", expr.span())?;
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
            let periods = positive_periods(&args[2], "Periods", "Finance Error", expr.span())?;
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
            let result = match name {
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
        _ => unreachable!(),
    }
}
