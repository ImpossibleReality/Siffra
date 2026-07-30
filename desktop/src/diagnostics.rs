//! When a diagnostic is worth showing.
//!
//! A calculation is wrong for most of the time it is being typed, and saying so
//! on every keystroke is noise rather than help. Two cases are held back: the
//! line the caret is finishing, and the first line of an empty document, where
//! a lone name is a calculation that has not been written yet rather than one
//! that refers to nothing.

use crate::engine::LineOutput;
use std::time::Duration;

/// How long the typing on a line settles before a diagnostic about it is shown.
/// Short enough to feel immediate once you stop, long enough that a half-typed
/// expression never gets to complain.
pub const ERROR_DELAY: Duration = Duration::from_millis(200);

/// The first line that is not blank and not inside a comment.
pub fn top_non_comment_line(lines: &[&str]) -> Option<usize> {
    let mut in_block_comment = false;

    for (number, line) in lines.iter().enumerate() {
        let bytes = line.as_bytes();
        let mut offset = 0;

        while offset < bytes.len() {
            if in_block_comment {
                match line[offset..].find("*/") {
                    Some(end) => {
                        in_block_comment = false;
                        offset += end + 2;
                    }
                    None => break,
                }
                continue;
            }

            while bytes.get(offset).is_some_and(u8::is_ascii_whitespace) {
                offset += 1;
            }
            if offset >= bytes.len() || line[offset..].starts_with("//") {
                break;
            }
            if line[offset..].starts_with("/*") {
                in_block_comment = true;
                offset += 2;
                continue;
            }

            return Some(number);
        }
    }

    None
}

pub fn should_hide_diagnostic(
    output: &LineOutput,
    line: usize,
    lines: &[&str],
    cursor_line: Option<usize>,
    cursor_line_at_end: Option<usize>,
) -> bool {
    let LineOutput::Error { message, .. } = output else {
        return false;
    };

    if message == "Incomplete expression" && Some(line) == cursor_line_at_end {
        return true;
    }

    message == "Name Error"
        && Some(line) == cursor_line
        && Some(line) == top_non_comment_line(lines)
        && lines.get(line).is_some_and(|text| {
            !text.is_empty()
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(message: &str) -> LineOutput {
        LineOutput::Error {
            span: None,
            message: message.to_string(),
            description: None,
            location: None,
        }
    }

    #[test]
    fn finds_the_first_line_with_something_on_it() {
        assert_eq!(top_non_comment_line(&["", "  ", "2 + 2"]), Some(2));
        assert_eq!(top_non_comment_line(&["// note", "2 + 2"]), Some(1));
        assert_eq!(top_non_comment_line(&["/* a", "b */ 2 + 2"]), Some(1));
        assert_eq!(top_non_comment_line(&["", "// only comments"]), None);
    }

    #[test]
    fn holds_back_an_incomplete_line_the_caret_is_still_on() {
        assert!(should_hide_diagnostic(
            &error("Incomplete expression"),
            0,
            &["2 +"],
            Some(0),
            Some(0)
        ));
        // Once the caret leaves the line, the diagnostic stands.
        assert!(!should_hide_diagnostic(
            &error("Incomplete expression"),
            0,
            &["2 +", ""],
            Some(1),
            Some(1)
        ));
    }

    #[test]
    fn holds_back_a_lone_name_on_the_opening_line() {
        assert!(should_hide_diagnostic(
            &error("Name Error"),
            0,
            &["total"],
            Some(0),
            None
        ));
        // Not once it is part of an expression.
        assert!(!should_hide_diagnostic(
            &error("Name Error"),
            0,
            &["total + 1"],
            Some(0),
            None
        ));
    }

    #[test]
    fn leaves_every_other_diagnostic_alone() {
        assert!(!should_hide_diagnostic(
            &error("Unit Error"),
            0,
            &["1 m + 1 s"],
            Some(0),
            Some(0)
        ));
    }
}
