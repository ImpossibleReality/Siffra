mod factorial;
mod finance;
mod numeric;
mod transcendental;
mod utility;

use super::helpers::*;
use super::temporal::{anchored_endpoints, instant_from};
use crate::error::SiffraExecutionError;
use crate::representations::{Expression, Float, Value};
use crate::{siffra_error, siffra_try};
use jiff::Span as JiffSpan;

pub(super) fn evaluate_function(
    name: &str,
    args: &[Value],
    expr: &Expression,
) -> Result<Value, SiffraExecutionError> {
    match name {
        "round" | "floor" | "ceil" | "trunc" | "sign" | "mod" | "min" | "max" | "clamp"
        | "mean" | "gcd" | "lcm" | "choose" | "permute" | "atan2" | "hypot" => {
            numeric::evaluate(name, args, expr)
        }
        "apy" | "ear" | "apr" | "fv" | "compound" | "pv" | "pmt" => {
            finance::evaluate(name, args, expr)
        }
        "gamma" | "factorial" => factorial::evaluate(name, args, expr),
        "root" | "log" | "ln" | "sqrt" | "sin" | "cos" | "tan" | "sinh" | "cosh" | "tanh"
        | "asin" | "acos" | "atan" | "exp" | "abs" => transcendental::evaluate(name, args, expr),
        "start" | "end" | "midpoint" | "simplify" | "expand" => utility::evaluate(name, args, expr),
        name => Err(siffra_error!(
            "Name Error",
            format!("The function '{}' is not defined", name),
            expr.function_name_span().or(expr.span())
        )),
    }
}
