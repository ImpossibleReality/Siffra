use crate::error::SiffraExecutionError;
use crate::representations::{Dimension, TimeZone, Value};
use jiff::Zoned;
use std::collections::BTreeMap;

#[derive(Clone)]
pub enum VariableValue {
    Error(SiffraExecutionError),
    Value(Value),
}

#[derive(Clone)]
pub struct SiffraState {
    variables: BTreeMap<String, VariableValue>,
    previous_value: Option<Value>,
    block_total: Option<Value>,
    block_total_error: Option<(Dimension, Dimension)>,
    timezone: TimeZone,
    /// Resolved once per pass so every line in a document agrees on "now".
    now: Zoned,
}

pub enum VariableAccessError {
    NotDefined,
    Error(SiffraExecutionError),
}

impl Default for SiffraState {
    fn default() -> Self {
        Self::new()
    }
}

impl SiffraState {
    pub fn new() -> Self {
        Self::in_timezone(TimeZone::system())
    }

    /// A pass pinned to an explicit zone, for documents that name one and for
    /// tests that must not depend on the machine's zone.
    pub fn in_timezone(timezone: TimeZone) -> Self {
        let now = Zoned::now().with_time_zone(timezone.inner().clone());
        Self {
            variables: BTreeMap::new(),
            previous_value: None,
            block_total: Some(Value::from(0.0)),
            block_total_error: None,
            timezone,
            now,
        }
    }

    pub fn timezone(&self) -> &TimeZone {
        &self.timezone
    }

    pub fn now(&self) -> &Zoned {
        &self.now
    }

    /// Pins the evaluation timestamp, so `now` is reproducible under test.
    pub fn set_now(&mut self, now: Zoned) {
        self.now = now;
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

    pub(super) fn block_total_error(&self) -> Option<&(Dimension, Dimension)> {
        self.block_total_error.as_ref()
    }

    pub fn add_to_block_total(&mut self, value: &Value) {
        if let Some(total) = self.block_total.take() {
            if total == Value::from(0.0) {
                self.block_total = Some(value.clone());
            } else {
                // Keep the running total on the left for percentage semantics.
                let total_unit = total.dimension();
                let value_unit = value.dimension();
                self.block_total = total.try_add(value);
                if self.block_total.is_none() {
                    self.block_total_error = Some((total_unit, value_unit));
                }
            }
        }
    }

    pub fn clear_block_total(&mut self) {
        self.block_total = Some(Value::from(0.0));
        self.block_total_error = None;
    }
}
