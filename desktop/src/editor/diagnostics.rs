//! Delayed evaluation and diagnostic visibility for the editor.

use super::Editor;
use crate::calculation::LineOutput;
use gpui::Context;
use std::time::{Duration, Instant};

/// How long the typing on a line settles before a diagnostic about it is shown.
pub(super) const ERROR_DELAY: Duration = Duration::from_millis(200);

/// The first line that is not blank and not inside a comment.
fn top_non_comment_line(lines: &[&str]) -> Option<usize> {
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

fn should_hide_diagnostic(
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

pub(super) fn carry_edit_times<T: std::marker::Copy>(
    previous: &[String],
    times: &[Option<T>],
    next: &[String],
    now: T,
) -> Vec<Option<T>> {
    let mut prefix = 0;
    while prefix < previous.len() && prefix < next.len() && previous[prefix] == next[prefix] {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < previous.len() - prefix
        && suffix < next.len() - prefix
        && previous[previous.len() - 1 - suffix] == next[next.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let mut edited = vec![Some(now); next.len()];
    for index in 0..prefix {
        edited[index] = times.get(index).copied().flatten();
    }
    for offset in 0..suffix {
        edited[next.len() - 1 - offset] = times.get(previous.len() - 1 - offset).copied().flatten();
    }
    edited
}

impl Editor {
    /// Re-runs the calculation. Every line is re-evaluated, but only the lines
    /// whose text or predecessors changed are actually recomputed.
    pub(super) fn recalculate(&mut self) {
        let lines: Vec<String> = self
            .text
            .content()
            .split('\n')
            .map(str::to_string)
            .collect();
        self.note_edits(lines);
        self.outputs = self.calculation.evaluate(self.text.content());
    }

    pub(super) fn note_edits(&mut self, next: Vec<String>) {
        self.line_edited_at = carry_edit_times(
            &self.previous_lines,
            &self.line_edited_at,
            &next,
            Instant::now(),
        );
        self.previous_lines = next;
    }

    /// How much of a line's settling time is left, or `None` once it has
    /// settled and its diagnostic may be shown.
    pub(super) fn remaining_delay(&self, line: usize, now: Instant) -> Option<Duration> {
        let edited = self.line_edited_at.get(line).copied().flatten()?;
        let elapsed = now.saturating_duration_since(edited);
        (elapsed < ERROR_DELAY).then(|| ERROR_DELAY - elapsed)
    }

    /// The line the caret is on, and whether it sits at that line's end — the
    /// two things a diagnostic's visibility turns on. A selection is not a
    /// caret and suppresses nothing.
    pub(super) fn caret_lines(&self) -> (Option<usize>, Option<usize>) {
        if !self.text.selection().is_empty() {
            return (None, None);
        }

        let caret = self.caret_offset();
        let mut start = 0;
        for (index, line) in self.text.content().split('\n').enumerate() {
            let end = start + line.len();
            if caret <= end {
                return (Some(index), (caret == end).then_some(index));
            }
            start = end + 1;
        }
        (None, None)
    }

    /// Decides which results the column shows.
    ///
    /// A calculation is wrong for most of the time it is being typed, and a
    /// diagnostic that appears on every keystroke is noise. An error is held
    /// back until the typing on its line has settled, and while it is held back
    /// whatever was already showing there stays put, so the column does not
    /// flicker under the caret. Values are never held back — an answer is worth
    /// having the moment there is one.
    pub(super) fn refresh_diagnostics(&mut self, cx: &mut Context<Self>) {
        let lines: Vec<&str> = self.text.content().split('\n').collect();
        let (cursor_line, cursor_line_at_end) = self.caret_lines();
        let now = Instant::now();
        let previous = &self.displayed;

        let mut soonest: Option<Duration> = None;
        let mut displayed = Vec::with_capacity(self.outputs.len());

        for (index, output) in self.outputs.iter().enumerate() {
            displayed.push(match output {
                None => None,
                Some(LineOutput::Value { .. }) => output.clone(),
                Some(error @ LineOutput::Error { message, .. }) => {
                    if should_hide_diagnostic(error, index, &lines, cursor_line, cursor_line_at_end)
                    {
                        None
                    } else if message == "Incomplete expression" {
                        // The caret has already left this line — that is the
                        // only way an incomplete one gets this far — so there
                        // is nothing left to wait for.
                        Some(error.clone())
                    } else if let Some(remaining) = self.remaining_delay(index, now) {
                        soonest = Some(
                            soonest.map_or(remaining, |soonest: Duration| soonest.min(remaining)),
                        );
                        previous
                            .get(index)
                            .cloned()
                            .flatten()
                            .filter(|shown| matches!(shown, LineOutput::Error { .. }))
                    } else {
                        Some(error.clone())
                    }
                }
            });
        }

        self.displayed = displayed;
        self.diagnostic_timer = soonest.map(|delay| {
            cx.spawn(async move |editor, cx| {
                cx.background_executor().timer(delay).await;
                editor
                    .update(cx, |editor, cx| editor.refresh_diagnostics(cx))
                    .ok();
            })
        });
        cx.notify();
    }
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
