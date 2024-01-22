use crate::representations::Value;
use std::collections::BTreeMap;

pub struct SiffraState {
    variables: BTreeMap<String, Value>,
}

impl SiffraState {
    pub fn new() -> Self {
        Self {
            variables: BTreeMap::new(),
        }
    }

    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    pub fn set_variable(&mut self, name: &str, value: Value) {
        self.variables.insert(name.to_string(), value);
    }
}
