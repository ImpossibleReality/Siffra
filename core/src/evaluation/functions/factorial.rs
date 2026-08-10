use super::*;

const MIN_FACTORIAL_INPUT: i32 = -1000;
const MAX_FACTORIAL_INPUT: i32 = 1000;

pub(super) const DEFINITIONS: &[Function] = siffra_functions![
    {
        name: "factorial",
        category: "Combinatorics",
        notations: ["x!"],
        params: (x),
        description: "Factorial of x, extended to non-integers via the gamma function.",
        examples: ["factorial(6)" => "720"],
        eval: factorial,
    },
    {
        name: "gamma",
        category: "Combinatorics",
        params: (x),
        description: "Gamma function Γ(x), with Γ(n) = (n - 1)! for positive integers.",
        examples: ["gamma(0.5)" => "1.772454"],
        eval: gamma,
    },
];

fn factorial(call: &Call) -> Result<Value, SiffraExecutionError> {
    let x = supported_input(call, "Factorial")?;
    if x < Float::from(0) && x.is_integer() {
        return Err(siffra_error!(
            "Factorial Error",
            "Factorial is undefined for negative integers",
            call.span()
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
}

fn gamma(call: &Call) -> Result<Value, SiffraExecutionError> {
    let x = supported_input(call, "Gamma")?;
    if x <= Float::from(0) && x.is_integer() {
        return Err(siffra_error!(
            "Gamma Error",
            "Gamma is undefined for zero and negative integers",
            call.span()
        ));
    }
    let result = if x.is_nonneg_integer() {
        factorial_exact(&(&x - &Float::from(1)))
    } else {
        x.gamma().round_significant_digits(14)
    };
    Ok(Value::new(result, None))
}

// Both grow fast enough that the supported range is the same, and both name
// themselves in their errors rather than using the spelling that was typed.
fn supported_input(call: &Call, label: &str) -> Result<Float, SiffraExecutionError> {
    let value = call.arg(0);
    if !value.dimension().is_unitless() {
        return Err(siffra_error!(
            "Unit Error",
            format!("{label} must be unitless; got {}", value_unit(value)),
            call.span()
        ));
    }
    let x = value.value();
    if x < Float::from(MIN_FACTORIAL_INPUT) || x > Float::from(MAX_FACTORIAL_INPUT) {
        return Err(siffra_error!(
            format!("{label} Error"),
            format!(
                "{label} is supported for inputs from {MIN_FACTORIAL_INPUT} through {MAX_FACTORIAL_INPUT}"
            ),
            call.span()
        ));
    }
    Ok(x)
}

pub fn factorial_exact(x: &Float) -> Float {
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
