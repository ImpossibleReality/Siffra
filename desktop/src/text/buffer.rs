use std::collections::VecDeque;
use std::ops::Range;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

const GROUP_TIMEOUT: Duration = Duration::from_millis(1500);
const MAX_ENTRIES: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditKind {
    Insert,
    DeleteBackward,
    DeleteForward,
    Compose,
    Discrete,
}

#[derive(Clone)]
struct Snapshot {
    content: String,
    selection: Range<usize>,
    selection_reversed: bool,
}

struct Group {
    kind: EditKind,
    end: usize,
    at: Instant,
}

struct History {
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    open: Option<Group>,
}

impl History {
    fn new() -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            open: None,
        }
    }

    fn edit(&mut self, before: Snapshot, kind: EditKind, range: Range<usize>, inserted: usize) {
        self.redo.clear();

        if !self.joins(kind, &range) {
            if self.undo.len() == MAX_ENTRIES {
                self.undo.pop_front();
            }
            self.undo.push_back(before);
        }

        self.open = (kind != EditKind::Discrete).then(|| Group {
            kind,
            end: range.start + inserted,
            at: Instant::now(),
        });
    }

    fn joins(&self, kind: EditKind, range: &Range<usize>) -> bool {
        let Some(open) = self.open.as_ref() else {
            return false;
        };
        if open.kind != kind {
            return false;
        }

        match kind {
            EditKind::Compose => true,
            EditKind::Discrete => false,
            _ if open.at.elapsed() >= GROUP_TIMEOUT => false,
            EditKind::Insert | EditKind::DeleteForward => range.start == open.end,
            EditKind::DeleteBackward => range.end == open.end,
        }
    }

    fn close_group(&mut self) {
        self.open = None;
    }

    fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let previous = self.undo.pop_back()?;
        self.redo.push(current);
        self.open = None;
        Some(previous)
    }

    fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let next = self.redo.pop()?;
        if self.undo.len() == MAX_ENTRIES {
            self.undo.pop_front();
        }
        self.undo.push_back(current);
        self.open = None;
        Some(next)
    }
}

pub struct TextBuffer {
    content: String,
    selection: Range<usize>,
    selection_reversed: bool,
    selection_prepared_for_edit: bool,
    marked_range: Option<Range<usize>>,
    history: History,
}

impl TextBuffer {
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let caret = content.len();
        Self {
            content,
            selection: caret..caret,
            selection_reversed: false,
            selection_prepared_for_edit: false,
            marked_range: None,
            history: History::new(),
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn selection(&self) -> &Range<usize> {
        &self.selection
    }

    pub fn selection_reversed(&self) -> bool {
        self.selection_reversed
    }

    pub fn marked_range(&self) -> Option<&Range<usize>> {
        self.marked_range.as_ref()
    }

    pub fn caret_offset(&self) -> usize {
        if self.selection_reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    pub fn reset(&mut self, content: impl Into<String>) {
        *self = Self::new(content);
    }

    pub fn close_history_group(&mut self) {
        self.history.close_group();
    }

    pub fn move_to(&mut self, offset: usize) {
        self.history.close_group();
        let offset = offset.min(self.content.len());
        self.selection = offset..offset;
        self.selection_reversed = false;
        self.selection_prepared_for_edit = false;
    }

    pub fn select_to(&mut self, offset: usize) {
        self.history.close_group();
        let offset = offset.min(self.content.len());
        if self.selection_reversed {
            self.selection.start = offset;
        } else {
            self.selection.end = offset;
        }
        if self.selection.end < self.selection.start {
            self.selection_reversed = !self.selection_reversed;
            self.selection = self.selection.end..self.selection.start;
        }
        self.selection_prepared_for_edit = false;
    }

    pub fn set_selection(&mut self, selection: Range<usize>, reversed: bool) {
        self.history.close_group();
        self.selection =
            selection.start.min(self.content.len())..selection.end.min(self.content.len());
        self.selection_reversed = reversed;
        self.selection_prepared_for_edit = false;
    }

    /// Selects a range for the next edit without closing its history group.
    pub fn set_selection_for_edit(&mut self, selection: Range<usize>) {
        self.selection =
            selection.start.min(self.content.len())..selection.end.min(self.content.len());
        self.selection_reversed = false;
        self.selection_prepared_for_edit = true;
    }

    pub fn select_all(&mut self) {
        self.history.close_group();
        self.selection = 0..self.content.len();
        self.selection_reversed = false;
        self.selection_prepared_for_edit = false;
    }

    pub fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    pub fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.content.len())
    }

    pub fn previous_word_boundary(&self, offset: usize) -> usize {
        self.content
            .split_word_bound_indices()
            .filter(|(start, word)| *start < offset && is_word(word))
            .next_back()
            .map_or(0, |(start, _)| start)
    }

    pub fn next_word_boundary(&self, offset: usize) -> usize {
        self.content
            .split_word_bound_indices()
            .map(|(start, word)| (start + word.len(), word))
            .find(|(end, word)| *end > offset && is_word(word))
            .map_or(self.content.len(), |(end, _)| end)
    }

    pub fn word_range_at(&self, offset: usize) -> Range<usize> {
        let mut preceding = offset..offset;
        for (start, word) in self.content.split_word_bound_indices() {
            let range = start..start + word.len();
            if range.start <= offset && offset < range.end {
                return range;
            }
            if range.end == offset {
                preceding = range;
            }
        }
        preceding
    }

    pub fn selected_text(&self) -> Option<&str> {
        (!self.selection.is_empty()).then(|| &self.content[self.selection.clone()])
    }

    pub fn replace_selection(&mut self, text: &str, kind: EditKind) {
        let range = self.selection.clone();
        let kind = if range.is_empty() || self.selection_prepared_for_edit {
            kind
        } else {
            EditKind::Discrete
        };

        self.history
            .edit(self.snapshot(), kind, range.clone(), text.len());
        self.content.replace_range(range.clone(), text);
        let caret = range.start + text.len();
        self.selection = caret..caret;
        self.selection_reversed = false;
        self.selection_prepared_for_edit = false;
        self.marked_range = None;
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.history.undo(self.snapshot()) else {
            return false;
        };
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.history.redo(self.snapshot()) else {
            return false;
        };
        self.restore(snapshot);
        true
    }

    pub fn unmark_text(&mut self) {
        self.marked_range = None;
    }

    pub fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, text: &str) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or_else(|| self.selection.clone());
        let kind = if self.marked_range.is_some() {
            EditKind::Compose
        } else if text.contains('\n') {
            EditKind::Discrete
        } else {
            EditKind::Insert
        };

        self.selection = range;
        self.selection_prepared_for_edit = false;
        self.replace_selection(text, kind);
        if kind == EditKind::Compose {
            self.history.close_group();
        }
    }

    pub fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or_else(|| self.selection.clone());

        self.history.edit(
            self.snapshot(),
            EditKind::Compose,
            range.clone(),
            text.len(),
        );
        self.content.replace_range(range.clone(), text);
        self.marked_range = (!text.is_empty()).then_some(range.start..range.start + text.len());
        self.selection = selected_utf16
            .as_ref()
            .map(|selected| self.range_from_utf16(selected))
            .map(|selected| selected.start + range.start..selected.end + range.start)
            .unwrap_or_else(|| {
                let caret = range.start + text.len();
                caret..caret
            });
        self.selection_prepared_for_edit = false;
    }

    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            content: self.content.clone(),
            selection: self.selection.clone(),
            selection_reversed: self.selection_reversed,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.content = snapshot.content;
        self.marked_range = None;
        self.selection.start = snapshot.selection.start.min(self.content.len());
        self.selection.end = snapshot.selection.end.min(self.content.len());
        self.selection_reversed = snapshot.selection_reversed;
        self.selection_prepared_for_edit = false;
    }
}

fn is_word(segment: &str) -> bool {
    segment.chars().any(char::is_alphanumeric)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_groups_into_one_undo_step() {
        let mut buffer = TextBuffer::new("");
        buffer.replace_selection("a", EditKind::Insert);
        buffer.replace_selection("b", EditKind::Insert);
        buffer.replace_selection("c", EditKind::Insert);

        assert!(buffer.undo());
        assert_eq!(buffer.content(), "");
        assert!(!buffer.undo());
    }

    #[test]
    fn moving_the_caret_splits_typing_groups() {
        let mut buffer = TextBuffer::new("");
        buffer.replace_selection("a", EditKind::Insert);
        buffer.move_to(0);
        buffer.move_to(1);
        buffer.replace_selection("b", EditKind::Insert);

        assert!(buffer.undo());
        assert_eq!(buffer.content(), "a");
        assert!(buffer.undo());
        assert_eq!(buffer.content(), "");
    }

    #[test]
    fn insert_and_delete_are_separate_undo_steps() {
        let mut buffer = TextBuffer::new("");
        buffer.replace_selection("ab", EditKind::Insert);
        buffer.set_selection_for_edit(1..2);
        buffer.replace_selection("", EditKind::DeleteBackward);
        buffer.set_selection_for_edit(0..1);
        buffer.replace_selection("", EditKind::DeleteBackward);

        assert!(buffer.undo());
        assert_eq!(buffer.content(), "ab");
        assert!(buffer.undo());
        assert_eq!(buffer.content(), "");
    }

    #[test]
    fn editing_after_undo_clears_redo() {
        let mut buffer = TextBuffer::new("");
        buffer.replace_selection("a", EditKind::Insert);
        assert!(buffer.undo());
        assert!(buffer.redo());
        assert!(buffer.undo());
        buffer.replace_selection("b", EditKind::Insert);
        assert!(!buffer.redo());
    }

    #[test]
    fn undo_and_redo_restore_selection() {
        let mut buffer = TextBuffer::new("x");
        buffer.replace_selection("y", EditKind::Discrete);
        assert!(buffer.undo());
        assert_eq!(buffer.content(), "x");
        assert_eq!(buffer.selection(), &(1..1));
        assert!(buffer.redo());
        assert_eq!(buffer.content(), "xy");
        assert_eq!(buffer.selection(), &(2..2));
    }

    #[test]
    fn history_is_capped() {
        let mut buffer = TextBuffer::new("");
        for _ in 0..MAX_ENTRIES + 10 {
            buffer.replace_selection("x", EditKind::Discrete);
        }
        assert_eq!(buffer.history.undo.len(), MAX_ENTRIES);
    }

    #[test]
    fn utf16_offsets_round_trip_non_ascii_text() {
        let buffer = TextBuffer::new("a😀é");
        let utf8 = 1..5;
        let utf16 = buffer.range_to_utf16(&utf8);
        assert_eq!(utf16, 1..3);
        assert_eq!(buffer.range_from_utf16(&utf16), utf8);
    }
}
