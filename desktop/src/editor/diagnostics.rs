use super::Editor;
use crate::calculation::LineOutput;
use gpui::Context;
use std::time::{Duration, Instant};

/// How long the typing on a line settles before a diagnostic about it is shown. Valid results are still shown instantly.
pub(super) const ERROR_DELAY: Duration = Duration::from_millis(200);

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

/// Reuses timestamps for unchanged prefix and suffix lines.
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

    pub(super) fn remaining_delay(&self, line: usize, now: Instant) -> Option<Duration> {
        let edited = self.line_edited_at.get(line).copied().flatten()?;
        let elapsed = now.saturating_duration_since(edited);
        (elapsed < ERROR_DELAY).then(|| ERROR_DELAY - elapsed)
    }

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
