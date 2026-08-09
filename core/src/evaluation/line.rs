use super::{SiffraState, evaluate_expr};
use crate::error::SiffraExecutionError;
use crate::parser::parse_line;
use crate::parser::representation::ParsedLine;
use crate::representations::Value;

pub type EvaluationResult = Result<Option<Value>, SiffraExecutionError>;

pub fn evaluate_line(line: &str, state: &mut SiffraState) -> EvaluationResult {
    let parsed_line = parse_line(line)?;
    match parsed_line {
        ParsedLine::Comment | ParsedLine::Header(_) => {
            state.clear_block_total();
            Ok(None)
        }
        ParsedLine::Expression(expr) => {
            let value = match evaluate_expr(&expr.try_into()?, state) {
                Ok(value) => value,
                Err(err) => {
                    state.clear_previous_value();
                    return Err(err);
                }
            };
            state.set_previous_value(value.clone());
            state.add_to_block_total(&value);
            Ok(Some(value))
        }
        ParsedLine::Variable(name, expr) => {
            let value = evaluate_expr(&expr.try_into()?, state);
            match value {
                Ok(value) => {
                    state.set_variable(&name, value.clone());
                    state.set_previous_value(value.clone());
                    state.add_to_block_total(&value);
                    Ok(Some(value))
                }
                Err(err) => {
                    state.error_variable(&name, err.clone());
                    state.clear_previous_value();
                    Err(err)
                }
            }
        }
    }
}
