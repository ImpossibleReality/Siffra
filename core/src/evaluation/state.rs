use crate::error::SiffraExecutionError;
use crate::representations::Value;
use std::collections::BTreeMap;

pub enum VariableValue {
    Error(SiffraExecutionError),
    Value(Value),
}

pub struct SiffraState {
    variables: BTreeMap<String, VariableValue>,
    previous_value: Option<Value>,
    block_total: Option<Value>,
}

pub enum VariableAccessError {
    NotDefined,
    Error(SiffraExecutionError),
}

impl SiffraState {
    pub fn new() -> Self {
        Self {
            variables: BTreeMap::new(),
            previous_value: None,
            block_total: Some(Value::from(0.0)),
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
        self.variables
            .insert(name.to_string(), VariableValue::Value(value));
    }

    pub fn error_variable(&mut self, name: &str, error: SiffraExecutionError) {
        self.variables
            .insert(name.to_string(), VariableValue::Error(error));
    }

    pub fn set_previous_value(&mut self, value: Value) {
        self.previous_value = Some(value);
    }

    pub fn previous_value(&self) -> Option<&Value> {
        self.previous_value.as_ref()
    }

    pub fn clear_previous_value(&mut self) {
        self.previous_value = None;
    }

    pub fn block_total(&self) -> &Option<Value> {
        &self.block_total
    }

    pub fn add_to_block_total(&mut self, value: &Value) {
        if let Some(total) = self.block_total.take() {
            if total == Value::from(0.0) {
                self.block_total = Some(value.clone());
            } else {
                self.block_total = value.try_add(&total);
            }
        }
    }

    pub fn clear_block_total(&mut self) {
        self.block_total = Some(Value::from(0.0));
    }
}
