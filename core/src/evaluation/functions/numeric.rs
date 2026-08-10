use super::*;

const MIN_ROUND_PLACES: i32 = -100;
const MAX_ROUND_PLACES: i32 = 100;
const MAX_DECIMAL_PLACE_INPUT: i32 = 1_000_000;

const MAX_COMBINATORIC_INPUT: i32 = 1_000;

pub(super) const DEFINITIONS: &[Function] = siffra_functions![
    {
        name: "round",
        category: "Rounding",
        params: (x; places),
        description: "Rounds x to the nearest integer or to the requested decimal places; keeps the unit of x.",
        examples: ["round(12.345, 2)" => "12.35", "round(1250, -2)" => "1,300"],
        eval: round,
    },
    {
        name: "floor",
        category: "Rounding",
        params: (x),
        description: "Rounds x down to the nearest integer; keeps its unit.",
        examples: ["floor(-1.2 kg)" => "-2 kg"],
        eval: floor,
    },
    {
        name: "ceil",
        category: "Rounding",
        params: (x),
        description: "Rounds x up to the nearest integer; keeps its unit.",
        examples: ["ceil(-1.2 kg)" => "-1 kg"],
        eval: ceil,
    },
    {
        name: "trunc",
        category: "Rounding",
        params: (x),
        description: "Removes the fractional part of x by rounding toward zero; keeps its unit.",
        examples: ["trunc(-1.9 kg)" => "-1 kg"],
        eval: trunc,
    },
    {
        name: "sign",
        category: "Arithmetic",
        params: (x),
        description: "Returns -1, 0, or 1 according to the sign of x.",
        examples: ["sign(-42 m)" => "-1"],
        eval: sign,
    },
    {
        name: "mod",
        category: "Arithmetic",
        params: (a, b),
        description: "Euclidean remainder after division; a and b must have compatible units.",
        examples: ["mod(5 m, 200 cm)" => "1 m"],
        eval: modulus,
    },
    {
        name: "hypot",
        category: "Arithmetic",
        params: (a, b),
        description: "Length of the hypotenuse formed by two values with compatible units.",
        examples: ["hypot(3 m, 400 cm)" => "5 m"],
        eval: hypot,
    },
    {
        name: "min",
        category: "Aggregation",
        params: (a, b; min 1),
        description: "The smallest of one or more values with compatible units.",
        examples: ["min(2 m, 150 cm, 3 m)" => "1.5 m"],
        eval: min,
    },
    {
        name: "max",
        category: "Aggregation",
        params: (a, b; min 1),
        description: "The largest of one or more values with compatible units.",
        examples: ["max(2 m, 150 cm, 3 m)" => "3 m"],
        eval: max,
    },
    {
        name: "mean",
        category: "Aggregation",
        params: (a, b; min 1),
        description: "Arithmetic mean of one or more values with compatible units.",
        examples: ["mean(1 m, 200 cm, 3 m)" => "2 m"],
        eval: mean,
    },
    {
        name: "clamp",
        category: "Aggregation",
        params: (x, low, high),
        description: "Limits x to an inclusive range whose bounds have compatible units.",
        examples: ["clamp(250 cm, 1 m, 2 m)" => "200 cm"],
        eval: clamp,
    },
    {
        name: "gcd",
        category: "Combinatorics",
        params: (a, b),
        description: "Greatest common divisor of two unitless integers.",
        examples: ["gcd(84, 30)" => "6"],
        eval: gcd,
    },
    {
        name: "lcm",
        category: "Combinatorics",
        params: (a, b),
        description: "Least common multiple of two unitless integers.",
        examples: ["lcm(84, 30)" => "420"],
        eval: lcm,
    },
    {
        name: "choose",
        aliases: ["nCr"],
        category: "Combinatorics",
        params: (n, k),
        description: "Number of ways to choose k items from n without regard to order.",
        examples: ["choose(10, 3)" => "120"],
        eval: choose,
    },
    {
        name: "permute",
        category: "Combinatorics",
        params: (n, k),
        description: "Number of ordered selections of k items from n.",
        examples: ["permute(10, 3)" => "720"],
        eval: permute,
    },
    {
        name: "atan2",
        category: "Trigonometry",
        params: (y, x),
        description: "Four-quadrant inverse tangent of y and x, returning radians.",
        examples: ["atan2(1, 1)" => "0.785398 rad"],
        eval: atan2,
    },
];

fn round(call: &Call) -> Result<Value, SiffraExecutionError> {
    let value = call.arg(0);
    let places = match call.args() {
        [_] => 0,
        [_, places, ..] => {
            if !places.dimension().is_unitless() {
                return Err(siffra_error!(
                    "Unit Error",
                    format!(
                        "Decimal places must be unitless; got {}",
                        value_unit(places)
                    ),
                    call.span()
                ));
            }
            bounded_integer(&places.value(), MIN_ROUND_PLACES, MAX_ROUND_PLACES).ok_or_else(
                || {
                    siffra_error!(
                        "Round Error",
                        format!(
                            "Decimal places must be an integer from {MIN_ROUND_PLACES} through {MAX_ROUND_PLACES}"
                        ),
                        call.span()
                    )
                },
            )?
        }
        [] => unreachable!("round takes at least one argument"),
    };
    Ok(Value::new(
        value.value().round_decimal_places(places),
        Some(value.dimension()),
    ))
}

fn floor(call: &Call) -> Result<Value, SiffraExecutionError> {
    Ok(Value::new(
        call.arg(0).value().floor(),
        Some(call.arg(0).dimension()),
    ))
}

fn ceil(call: &Call) -> Result<Value, SiffraExecutionError> {
    Ok(Value::new(
        call.arg(0).value().ceil(),
        Some(call.arg(0).dimension()),
    ))
}

fn trunc(call: &Call) -> Result<Value, SiffraExecutionError> {
    Ok(Value::new(
        call.arg(0).value().trunc(),
        Some(call.arg(0).dimension()),
    ))
}

fn sign(call: &Call) -> Result<Value, SiffraExecutionError> {
    let zero = Float::from(0);
    let value = call.arg(0).value();
    let sign = if value < zero {
        -1
    } else if value > zero {
        1
    } else {
        0
    };
    Ok(Value::new(Float::from(sign), None))
}

fn modulus(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (dimension, values) = values_in_first_unit(call.args(), call.name(), call.span())?;
    let remainder = modulo(&values[0], &values[1]).ok_or_else(|| {
        siffra_error!("Division Error", "The modulus cannot be zero", call.span())
    })?;
    Ok(Value::new(remainder, Some(dimension)))
}

fn hypot(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (dimension, values) = values_in_first_unit(call.args(), call.name(), call.span())?;
    let squared = &(&values[0] * &values[0]) + &(&values[1] * &values[1]);
    Ok(Value::new(squared.sqrt().round_inexact(), Some(dimension)))
}

fn min(call: &Call) -> Result<Value, SiffraExecutionError> {
    extremum(call, true)
}

fn max(call: &Call) -> Result<Value, SiffraExecutionError> {
    extremum(call, false)
}

fn extremum(call: &Call, smallest: bool) -> Result<Value, SiffraExecutionError> {
    let (dimension, values) = values_in_first_unit(call.args(), call.name(), call.span())?;
    let mut selected = values[0].clone();
    for value in values.iter().skip(1) {
        let replace = if smallest {
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

fn mean(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (dimension, values) = values_in_first_unit(call.args(), call.name(), call.span())?;
    let mut total = Float::from(0);
    for value in &values {
        total = &total + value;
    }
    Ok(Value::new(
        &total / &Float::from(values.len() as i64),
        Some(dimension),
    ))
}

fn clamp(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (dimension, values) = values_in_first_unit(call.args(), call.name(), call.span())?;
    let value = &values[0];
    let low = &values[1];
    let high = &values[2];
    if low > high {
        return Err(siffra_error!(
            "Clamp Error",
            "The lower bound cannot be greater than the upper bound",
            call.span()
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

fn gcd(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (lhs, rhs) = integer_pair(call)?;
    Ok(Value::new(gcd_float(&lhs, &rhs), None))
}

fn lcm(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (lhs, rhs) = integer_pair(call)?;
    let gcd = gcd_float(&lhs, &rhs);
    let result = if gcd == Float::from(0) {
        gcd
    } else {
        (&(&lhs / &gcd) * &rhs).abs()
    };
    Ok(Value::new(result, None))
}

fn integer_pair(call: &Call) -> Result<(Float, Float), SiffraExecutionError> {
    let name = call.name();
    require_unitless_args(call)?;
    let integers = call
        .args()
        .iter()
        .map(|arg| {
            bounded_integer(
                &arg.value(),
                -MAX_DECIMAL_PLACE_INPUT,
                MAX_DECIMAL_PLACE_INPUT,
            )
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            siffra_error!(
                "Integer Error",
                format!(
                    "{name} arguments must be integers from -{MAX_DECIMAL_PLACE_INPUT} through {MAX_DECIMAL_PLACE_INPUT}"
                ),
                call.span()
            )
        })?;
    Ok((Float::from(integers[0]), Float::from(integers[1])))
}

fn choose(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (n, k) = selection_counts(call)?;
    let mut result = Float::from(1);
    let k = k.min(n - k);
    for i in 1..=k {
        result = &result * &Float::from(n - k + i);
        result = &result / &Float::from(i);
    }
    Ok(Value::new(result, None))
}

fn permute(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (n, k) = selection_counts(call)?;
    let mut result = Float::from(1);
    for i in 0..k {
        result = &result * &Float::from(n - i);
    }
    Ok(Value::new(result, None))
}

fn selection_counts(call: &Call) -> Result<(i32, i32), SiffraExecutionError> {
    let name = call.name();
    require_unitless_args(call)?;
    let n = bounded_integer(&call.arg(0).value(), 0, MAX_COMBINATORIC_INPUT);
    let k = bounded_integer(&call.arg(1).value(), 0, MAX_COMBINATORIC_INPUT);
    let (Some(n), Some(k)) = (n, k) else {
        return Err(siffra_error!(
            "Integer Error",
            format!(
                "{name} arguments must be non-negative integers no greater than {MAX_COMBINATORIC_INPUT}"
            ),
            call.span()
        ));
    };
    if k > n {
        return Err(siffra_error!(
            "Domain Error",
            "k cannot be greater than n",
            call.span()
        ));
    }
    Ok((n, k))
}

fn require_unitless_args(call: &Call) -> Result<(), SiffraExecutionError> {
    if let Some((index, arg)) = call
        .args()
        .iter()
        .enumerate()
        .find(|(_, arg)| !arg.dimension().is_unitless())
    {
        return Err(siffra_error!(
            "Unit Error",
            format!(
                "{} argument {} must be unitless; got {}",
                call.name(),
                index + 1,
                value_unit(arg)
            ),
            call.span()
        ));
    }
    Ok(())
}

fn atan2(call: &Call) -> Result<Value, SiffraExecutionError> {
    let (_, values) = values_in_first_unit(call.args(), call.name(), call.span())?;
    let angle = values[0].atan2(&values[1]).ok_or_else(|| {
        siffra_error!(
            "Domain Error",
            "atan2 is undefined when both arguments are zero",
            call.span()
        )
    })?;
    Ok(radian_value(angle.round_inexact()))
}
