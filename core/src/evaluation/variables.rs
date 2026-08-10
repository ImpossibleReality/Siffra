use super::SiffraState;
use super::helpers::unit_label;
use super::state::VariableAccessError;
use crate::error::SiffraExecutionError;
use crate::parser::Span;
use crate::representations::Value;
use crate::siffra_error;

pub(super) fn resolve_variable(
    name: &str,
    state: &SiffraState,
    span: Option<Span>,
) -> Result<Value, SiffraExecutionError> {
    match state.get_variable(name) {
        Ok(value) => return Ok(value.clone()),
        Err(VariableAccessError::Error(source)) => {
            let cause = source.description().unwrap_or_else(|| source.message());
            return Err(siffra_error!(
                "Variable Error",
                format!("Variable '{name}' is invalid: {cause}"),
                span
            ));
        }
        Err(VariableAccessError::NotDefined) => {}
    }

    if let Some(constant) = crate::registry::constant(name) {
        return Ok((constant.value)());
    }

    // What is left reads the state of the document rather than naming a fixed
    // value, so it is resolved here instead of in the constant definitions.
    match name {
        "prev" | "ans" | "previous" | "that" => {
            if let Some(prev) = state.previous_value() {
                Ok(prev.clone())
            } else {
                Err(siffra_error!(
                    "Name Error",
                    "No previous value to use",
                    span
                ))
            }
        }
        "total" | "sum" => {
            if let Some(total) = state.block_total() {
                Ok(total.clone())
            } else {
                let description = state.block_total_error().map_or_else(
                    || "Block total is unavailable".to_string(),
                    |(total, value)| {
                        format!(
                            "Block total cannot mix {} and {}",
                            unit_label(total),
                            unit_label(value)
                        )
                    },
                );
                Err(siffra_error!("Total Error", description, span))
            }
        }
        _ => Err(siffra_error!(
            "Name Error",
            format!("Variable '{}' not found", name),
            span
        )),
    }
}
