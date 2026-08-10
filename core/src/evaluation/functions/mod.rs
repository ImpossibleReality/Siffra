mod factorial;
mod finance;
mod numeric;
mod transcendental;
mod utility;

use super::helpers::*;
use super::temporal::{anchored_endpoints, instant_from};
use crate::error::SiffraExecutionError;
use crate::registry::{Call, Function, siffra_functions};
use crate::representations::{Expression, Float, Value};
use crate::{siffra_error, siffra_try};
use jiff::Span as JiffSpan;

/// Every module's definitions. Documentation order is by category; within one
/// category, the order here and then within the module decides.
pub(crate) const DEFINITIONS: &[&[Function]] = &[
    transcendental::DEFINITIONS,
    numeric::DEFINITIONS,
    finance::DEFINITIONS,
    factorial::DEFINITIONS,
    utility::DEFINITIONS,
];

pub(super) fn evaluate_function(
    name: &str,
    args: &[Value],
    expr: &Expression,
) -> Result<Value, SiffraExecutionError> {
    let Some(function) = crate::registry::function(name) else {
        return Err(siffra_error!(
            "Name Error",
            format!("The function '{}' is not defined", name),
            expr.function_name_span().or(expr.span())
        ));
    };

    let call = Call::new(function, name, args, expr);
    call.check_arity()?;
    (function.eval)(&call)
}
