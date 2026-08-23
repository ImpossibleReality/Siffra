mod diagnostics;
mod grammar;
mod lexical;
pub mod representation;
mod span;
mod syntax;
mod temporal;
mod units;

#[cfg(test)]
mod tests;

pub use representation::*;
pub use span::Span;
pub use syntax::{SyntaxKind, SyntaxSpan, syntax_spans};

use crate::error::SiffraExecutionError;
use crate::siffra_error;
use chumsky::Parser;

/// Parses a `#`-prefixed header line.
pub fn parse_header(input: &str) -> Option<String> {
    let rest = input.trim_start().strip_prefix('#')?;
    Some(rest.trim_start_matches('#').trim().to_string())
}

/// Parses one line into an expression, assignment, header, or comment.
pub fn parse_line(input: &str) -> Result<ParsedLine, SiffraExecutionError> {
    if let Some(title) = parse_header(input) {
        return Ok(ParsedLine::Header(title));
    }

    let cleaned = lexical::blank_comments(input);
    if cleaned.trim().is_empty() {
        return Ok(ParsedLine::Comment);
    }

    grammar::line_parser()
        .parse(cleaned.as_str())
        .into_result()
        .map_err(|errs| {
            let err = errs
                .into_iter()
                .max_by_key(|e| e.span().end)
                .expect("parse failure always yields at least one error");
            let span = *err.span();
            let (message, description) = diagnostics::describe_error(&err, &cleaned);
            siffra_error!(message, description, Some(Span::new(span.start, span.end)))
        })
}
