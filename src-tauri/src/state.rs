pub struct TaleState {
    variables: BTreeMap<String, Value>,
}

impl TaleState {
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