use crate::representations::Value;
use std::collections::BTreeMap;
use crate::error::SiffraExecutionError;

pub enum VariableValue {
    Error(SiffraExecutionError),
    Value(Value),
}

pub struct SiffraState {
    variables: BTreeMap<String, VariableValue>,
}

pub enum VariableAccessError {
    NotDefined,
    Error(SiffraExecutionError),
}

impl SiffraState {
    pub fn new() -> Self {
        Self {
            variables: BTreeMap::new(),
        }
    }

    pub fn get_variable(&self, name: &str) -> Result<&Value, VariableAccessError> {
        match self.variables.get(name) {
            Some(VariableValue::Value(value)) => Ok(value),
            Some(VariableValue::Error(err)) => Err(VariableAccessError::Error(err.clone())),
            None => Err(VariableAccessError::NotDefined),
        }
    }

    pub fn set_variable(&mut self, name: &str, value: Value) {
        self.variables.insert(name.to_string(), VariableValue::Value(value));
    }

    pub fn error_variable(&mut self, name: &str, error: SiffraExecutionError) {
        self.variables.insert(name.to_string(), VariableValue::Error(error));
    }
}
