mod macros;

#[derive(Debug, Clone)]
pub struct SiffraExecutionError {
    message: String,
    description: Option<String>,
    span: Option<(usize, usize)>,
    location: Option<(String, u32)>,
}

impl SiffraExecutionError {
    pub fn new(message: String) -> SiffraExecutionError {
        SiffraExecutionError {
            message,
            description: None,
            span: None,
            location: None,
        }
    }

    pub fn with_description(mut self, description: String) -> SiffraExecutionError {
        self.description = Some(description);
        self
    }

    pub fn with_span(mut self, start: usize, end: usize) -> SiffraExecutionError {
        self.span = Some((start, end));
        self
    }

    pub fn with_location(mut self, file: String, line: u32) -> SiffraExecutionError {
        self.location = Some((file, line));
        self
    }

    pub fn message(&self) -> String {
        self.message.clone()
    }
    pub fn description(&self) -> Option<String> {
        self.description.clone()
    }
    pub fn span(&self) -> Option<(usize, usize)> {
        self.span
    }
    pub fn location(&self) -> Option<String> {
        match &self.location {
            Some((file, line)) => Some(format!("{}:{}", file, line)),
            None => None,
        }
    }
}
