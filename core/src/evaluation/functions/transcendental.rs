use super::*;

pub(super) const DEFINITIONS: &[Function] = siffra_functions![
    {
        name: "sqrt",
        category: "Powers & logarithms",
        params: (x),
        description: "Square root of x.",
        examples: ["sqrt(144)" => "12"],
        eval: sqrt,
    },
    {
        name: "root",
        category: "Powers & logarithms",
        base: "n",
        params: (x),
        description: "The nth root of x. Write the degree after an underscore; one argument gives the square root.",
        examples: ["root(144)" => "12", "root_3(27)" => "3"],
        eval: root,
    },
    {
        name: "log",
        category: "Powers & logarithms",
        base: "b",
        params: (x),
        description: "Base-10 logarithm, or base-b when you write the base after an underscore.",
        examples: ["log(1000)" => "3", "log_2(32)" => "5"],
        eval: log,
    },
    {
        name: "ln",
        category: "Powers & logarithms",
        params: (x),
        description: "Natural logarithm (base e).",
        examples: ["ln(e)" => "1"],
        eval: ln,
    },
    {
        name: "exp",
        category: "Powers & logarithms",
        params: (x),
        description: "e raised to the power of x.",
        examples: ["exp(1)" => "2.718282"],
        eval: exp,
    },
    {
        name: "abs",
        category: "Arithmetic",
        params: (x),
        description: "Absolute value; keeps the unit of x.",
        examples: ["abs(-42)" => "42"],
        eval: abs,
    },
    {
        name: "sin",
        category: "Trigonometry",
        params: (x),
        description: "Sine of x (x in radians).",
        examples: ["sin(pi / 2)" => "1"],
        eval: sin,
    },
    {
        name: "cos",
        category: "Trigonometry",
        params: (x),
        description: "Cosine of x (x in radians).",
        examples: ["cos(0)" => "1"],
        eval: cos,
    },
    {
        name: "tan",
        category: "Trigonometry",
        params: (x),
        description: "Tangent of x (x in radians).",
        examples: ["tan(pi / 4)" => "1"],
        eval: tan,
    },
    {
        name: "sinh",
        category: "Trigonometry",
        params: (x),
        description: "Hyperbolic sine of a unitless x.",
        examples: ["sinh(1)" => "1.175201"],
        eval: sinh,
    },
    {
        name: "cosh",
        category: "Trigonometry",
        params: (x),
        description: "Hyperbolic cosine of a unitless x.",
        examples: ["cosh(1)" => "1.543081"],
        eval: cosh,
    },
    {
        name: "tanh",
        category: "Trigonometry",
        params: (x),
        description: "Hyperbolic tangent of a unitless x.",
        examples: ["tanh(1)" => "0.761594"],
        eval: tanh,
    },
    {
        name: "asin",
        category: "Trigonometry",
        params: (x),
        description: "Inverse sine, returning radians.",
        examples: ["asin(1)" => "1.570796 rad"],
        eval: asin,
    },
    {
        name: "acos",
        category: "Trigonometry",
        params: (x),
        description: "Inverse cosine, returning radians.",
        examples: ["acos(0)" => "1.570796 rad"],
        eval: acos,
    },
    {
        name: "atan",
        category: "Trigonometry",
        params: (x),
        description: "Inverse tangent, returning radians.",
        examples: ["atan(1)" => "0.785398 rad"],
        eval: atan,
    },
];

fn sqrt(call: &Call) -> Result<Value, SiffraExecutionError> {
    let value = call.arg(0);
    if value.value() < Float::from(0) {
        return Err(siffra_error!(
            "Domain Error",
            "sqrt requires a non-negative value",
            call.span()
        ));
    }
    Ok(siffra_try!(
        value.try_pow(&Value::new(Float::from(0.5), None)).ok_or(()),
        "Root Error",
        "Error taking square root",
        call.span()
    ))
}

fn root(call: &Call) -> Result<Value, SiffraExecutionError> {
    // Without a degree this is a square root; with one the degree arrives first,
    // whether it was written `root_3(27)` or `root(3, 27)`.
    let [degree, value] = match call.args() {
        [value] => {
            if value.value() < Float::from(0) {
                return Err(siffra_error!(
                    "Domain Error",
                    "A square root requires a non-negative value",
                    call.span()
                ));
            }
            return Ok(siffra_try!(
                value.try_pow(&Value::new(Float::from(0.5), None)).ok_or(()),
                "Root Error",
                "Error taking root",
                call.span()
            ));
        }
        [degree, value] => [degree, value],
        _ => unreachable!("the declared arity allows one or two arguments"),
    };

    let degree = require_unitless(degree, "The root degree", call.span())?;
    if degree == Float::from(0) {
        return Err(siffra_error!(
            "Domain Error",
            "The root degree cannot be zero",
            call.span()
        ));
    }
    let exponent = Value::new(&Float::from(1) / &degree, None);

    if value.value() < Float::from(0) {
        let odd_integer_degree =
            degree.is_integer() && modulo(&degree.abs(), &Float::from(2)) == Some(Float::from(1));
        if !odd_integer_degree {
            return Err(siffra_error!(
                "Domain Error",
                "A negative value requires an odd integer root degree",
                call.span()
            ));
        }
        let positive = Value::new(value.value().abs(), Some(value.dimension()));
        return Ok(siffra_try!(
            positive.try_pow(&exponent).ok_or(()),
            "Root Error",
            "Error taking root",
            call.span()
        )
        .negate());
    }

    Ok(siffra_try!(
        value.try_pow(&exponent).ok_or(()),
        "Root Error",
        "Error taking root",
        call.span()
    ))
}

fn log(call: &Call) -> Result<Value, SiffraExecutionError> {
    let name = call.name();
    match call.args() {
        [value] => {
            let value = require_unitless(value, name, call.span())?;
            if value <= Float::from(0) {
                return Err(siffra_error!(
                    "Domain Error",
                    "log requires a positive number",
                    call.span()
                ));
            }
            Ok(Value::new(value.log10().round_inexact(), None))
        }
        [base, value] => {
            let base = require_unitless(base, name, call.span())?;
            let value = require_unitless(value, name, call.span())?;
            if base <= Float::from(0) || base == Float::from(1) || value <= Float::from(0) {
                return Err(siffra_error!(
                    "Domain Error",
                    "The logarithm base must be positive and not 1, and its argument must be positive",
                    call.span()
                ));
            }
            Ok(Value::new((&value.ln() / &base.ln()).round_inexact(), None))
        }
        _ => unreachable!("the declared arity allows one or two arguments"),
    }
}

fn ln(call: &Call) -> Result<Value, SiffraExecutionError> {
    let value = require_unitless(call.arg(0), call.name(), call.span())?;
    if value <= Float::from(0) {
        return Err(siffra_error!(
            "Domain Error",
            "ln requires a positive number",
            call.span()
        ));
    }
    Ok(Value::new(value.ln().round_inexact(), None))
}

fn exp(call: &Call) -> Result<Value, SiffraExecutionError> {
    let value = require_unitless(call.arg(0), call.name(), call.span())?;
    Ok(Value::new(value.exp().round_inexact(), None))
}

fn abs(call: &Call) -> Result<Value, SiffraExecutionError> {
    Ok(Value::new(
        call.arg(0).value().abs(),
        Some(call.arg(0).dimension()),
    ))
}

fn sin(call: &Call) -> Result<Value, SiffraExecutionError> {
    circular(call, Float::sin)
}

fn cos(call: &Call) -> Result<Value, SiffraExecutionError> {
    circular(call, Float::cos)
}

fn tan(call: &Call) -> Result<Value, SiffraExecutionError> {
    circular(call, Float::tan)
}

fn circular(call: &Call, f: fn(&Float) -> Float) -> Result<Value, SiffraExecutionError> {
    let radians = trig_radians(call.arg(0), call.name(), call.span())?;
    Ok(Value::new(round_trig(f(&radians), &radians), None))
}

fn sinh(call: &Call) -> Result<Value, SiffraExecutionError> {
    hyperbolic(call, Float::sinh)
}

fn cosh(call: &Call) -> Result<Value, SiffraExecutionError> {
    hyperbolic(call, Float::cosh)
}

fn tanh(call: &Call) -> Result<Value, SiffraExecutionError> {
    hyperbolic(call, Float::tanh)
}

fn hyperbolic(call: &Call, f: fn(&Float) -> Float) -> Result<Value, SiffraExecutionError> {
    let value = call.arg(0);
    if !value.dimension().is_unitless() {
        return Err(siffra_error!(
            "Unit Error",
            format!(
                "{} must be unitless; got {}",
                call.name(),
                value_unit(value)
            ),
            call.span()
        ));
    }
    Ok(Value::new(f(&value.value()).round_inexact(), None))
}

fn asin(call: &Call) -> Result<Value, SiffraExecutionError> {
    let input = bounded_inverse_input(call)?;
    Ok(radian_value(input.asin().round_inexact()))
}

fn acos(call: &Call) -> Result<Value, SiffraExecutionError> {
    let input = bounded_inverse_input(call)?;
    Ok(radian_value(input.acos().round_inexact()))
}

fn atan(call: &Call) -> Result<Value, SiffraExecutionError> {
    let input = require_unitless(call.arg(0), call.name(), call.span())?;
    Ok(radian_value(input.atan().round_inexact()))
}

// asin and acos are only real on [-1, 1].
fn bounded_inverse_input(call: &Call) -> Result<Float, SiffraExecutionError> {
    let input = require_unitless(call.arg(0), call.name(), call.span())?;
    if input < Float::from(-1) || input > Float::from(1) {
        return Err(siffra_error!(
            "Domain Error",
            format!("{} requires a value from -1 through 1", call.name()),
            call.span()
        ));
    }
    Ok(input)
}
