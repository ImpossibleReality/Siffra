use chumsky::error::{RichPattern, RichReason};
use chumsky::prelude::Rich;

pub(super) fn describe_error(err: &Rich<'_, char>, input: &str) -> (String, String) {
    if is_incomplete_assignment(input) {
        return (
            "Incomplete expression".to_string(),
            "Expected an expression after '='".to_string(),
        );
    }

    match err.reason() {
        RichReason::Custom(msg) => ("Syntax Error".to_string(), msg.clone()),
        RichReason::ExpectedFound { expected, .. } => match err.found() {
            Some(c) => (
                format!("Unexpected '{}'", c),
                format!("The character '{}' isn't valid here", c),
            ),
            None => {
                if let Some(closing) = expected.iter().find_map(closing_delimiter) {
                    return (
                        "Unclosed bracket".to_string(),
                        format!("Missing a closing '{}'", closing),
                    );
                }

                let trimmed = input.trim_end();
                let tail = trimmed.chars().last();
                match tail {
                    Some(op) if "+-*/^".contains(op) => (
                        "Incomplete expression".to_string(),
                        format!("Expected something after '{}'", op),
                    ),
                    _ => (
                        "Incomplete expression".to_string(),
                        "The line ended unexpectedly".to_string(),
                    ),
                }
            }
        },
    }
}

fn closing_delimiter(pattern: &RichPattern<'_, char>) -> Option<char> {
    match pattern {
        RichPattern::Token(token) => matches!(**token, ')' | ']' | '}').then(|| **token),
        _ => None,
    }
}

fn is_incomplete_assignment(input: &str) -> bool {
    let Some(lhs) = input.trim_end().strip_suffix('=') else {
        return false;
    };
    let lhs = lhs.trim();
    let name = match lhs.strip_prefix("let") {
        Some(rest) if rest.starts_with(char::is_whitespace) => rest.trim(),
        _ => lhs,
    };

    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
