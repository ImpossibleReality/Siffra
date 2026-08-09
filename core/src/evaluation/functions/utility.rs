use super::*;

pub(super) fn evaluate(
    name: &str,
    args: &[Value],
    expr: &Expression,
) -> Result<Value, SiffraExecutionError> {
    match name {
        "start" | "end" | "midpoint" => {
            if args.len() != 1 {
                return Err(siffra_error!(
                    "Argument Error",
                    format!("{name} takes exactly 1 argument"),
                    expr.span()
                ));
            }
            let (start, end) = anchored_endpoints(&args[0], name, expr.span())?;
            let moment = match name {
                "start" => start,
                "end" => end,
                _ => {
                    let half = start
                        .until(&end)
                        .ok()
                        .and_then(|span| span.total(jiff::Unit::Second).ok())
                        .map(|seconds| seconds / 2.0)
                        .ok_or_else(|| {
                            siffra_error!(
                                "Range Error",
                                "This range is too long to halve",
                                expr.span()
                            )
                        })?;
                    start
                        .checked_add(
                            JiffSpan::new().nanoseconds((half * 1_000_000_000.0).round() as i64),
                        )
                        .map_err(|_| {
                            siffra_error!(
                                "Range Error",
                                "The midpoint falls off the calendar",
                                expr.span()
                            )
                        })?
                }
            };
            Ok(instant_from(&moment))
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
                Some(named) => Ok(args[0].convert(&named).unwrap_or_else(|| args[0].clone())),
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

        _ => unreachable!(),
    }
}
