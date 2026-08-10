use super::*;

pub(super) const DEFINITIONS: &[Function] = siffra_functions![
    {
        name: "simplify",
        category: "Units",
        params: (x),
        description: "Collapses a compound unit into the named derived unit that matches it, when exactly one does (kg*m/s^2 -> N, J/s -> W). Leaves x unchanged when no single named unit fits.",
        examples: ["simplify(1000 kg*m/s^2)" => "1,000 N"],
        eval: simplify,
    },
    {
        name: "expand",
        category: "Units",
        params: (x),
        description: "Expands named derived units into the base units they are built from (N -> kg*m/s^2). The magnitude adjusts so the quantity is unchanged.",
        examples: ["expand(5 N)" => "5 kg*m/s^2"],
        eval: expand,
    },
    {
        name: "start",
        category: "Dates & ranges",
        params: (range),
        description: "The earlier endpoint of a range between two dates or two times.",
        examples: ["start(1/1/2026 to 1/31/2026)" => "01-01-2026"],
        eval: start,
    },
    {
        name: "end",
        category: "Dates & ranges",
        params: (range),
        description: "The later endpoint of a range between two dates or two times.",
        examples: ["end(1/1/2026 to 1/31/2026)" => "01-31-2026"],
        eval: end,
    },
    {
        name: "midpoint",
        category: "Dates & ranges",
        params: (range),
        description: "The moment halfway between the endpoints of a range.",
        examples: ["midpoint(1/1/2026 to 1/31/2026)" => "01-16-2026"],
        eval: midpoint,
    },
];

fn simplify(call: &Call) -> Result<Value, SiffraExecutionError> {
    let value = call.arg(0);
    match value.dimension().collapse_to_named() {
        Some(named) => Ok(value.convert(&named).unwrap_or_else(|| value.clone())),
        None => Ok(value.clone()),
    }
}

fn expand(call: &Call) -> Result<Value, SiffraExecutionError> {
    let value = call.arg(0);
    let expanded = value.dimension().expand();
    Ok(value.convert(&expanded).unwrap_or_else(|| value.clone()))
}

fn start(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (start, _) = anchored_endpoints(call.arg(0), call.name(), call.span())?;
    Ok(instant_from(&start))
}

fn end(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (_, end) = anchored_endpoints(call.arg(0), call.name(), call.span())?;
    Ok(instant_from(&end))
}

fn midpoint(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (start, end) = anchored_endpoints(call.arg(0), call.name(), call.span())?;
    let half = start
        .until(&end)
        .ok()
        .and_then(|span| span.total(jiff::Unit::Second).ok())
        .map(|seconds| seconds / 2.0)
        .ok_or_else(|| {
            siffra_error!(
                "Range Error",
                "This range is too long to halve",
                call.span()
            )
        })?;
    let moment = start
        .checked_add(JiffSpan::new().nanoseconds((half * 1_000_000_000.0).round() as i64))
        .map_err(|_| {
            siffra_error!(
                "Range Error",
                "The midpoint falls off the calendar",
                call.span()
            )
        })?;
    Ok(instant_from(&moment))
}
