use super::Editor;
use super::interaction::CopyFlash;
use crate::calculation::LineOutput;
use crate::text::{
    Backspace, Copy, Cut, Delete, DeleteToLineStart, DeleteWordLeft, DeleteWordRight, EditKind,
    End, Home, Left, Paste, Redo, Right, SelectAll, SelectEnd, SelectHome, SelectLeft, SelectRight,
    SelectWordLeft, SelectWordRight, Undo, WordLeft, WordRight,
};
use gpui::{ClipboardItem, Context, Window, actions};
use std::time::Instant;

actions!(
    siffra,
    [
        Up,
        Down,
        SelectUp,
        SelectDown,
        DocumentStart,
        DocumentEnd,
        Newline,
        ShowCharacterPalette,
    ]
);

impl Editor {
    pub(super) fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.undo() {
            self.after_restore(cx);
        }
    }

    pub(super) fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.redo() {
            self.after_restore(cx);
        }
    }

    pub(super) fn after_restore(&mut self, cx: &mut Context<Self>) {
        self.text_changed(cx);
    }

    pub(super) fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if self.text.selection().is_empty() {
            self.move_to(self.previous_boundary(self.caret_offset()), cx);
        } else {
            self.move_to(self.text.selection().start, cx);
        }
    }

    pub(super) fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if self.text.selection().is_empty() {
            self.move_to(self.next_boundary(self.caret_offset()), cx);
        } else {
            self.move_to(self.text.selection().end, cx);
        }
    }

    pub(super) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.select_to(self.previous_boundary(self.caret_offset()), cx);
    }

    pub(super) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.select_to(self.next_boundary(self.caret_offset()), cx);
    }

    pub(super) fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.move_to(self.previous_word_boundary(self.caret_offset()), cx);
    }

    pub(super) fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.move_to(self.next_word_boundary(self.caret_offset()), cx);
    }

    pub(super) fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferred_x = None;
        self.select_to(self.previous_word_boundary(self.caret_offset()), cx);
    }

    pub(super) fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferred_x = None;
        self.select_to(self.next_word_boundary(self.caret_offset()), cx);
    }

    pub(super) fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.text.selection().is_empty() {
            let caret = self.caret_offset();
            self.text
                .set_selection_for_edit(self.previous_word_boundary(caret)..caret);
        }
        self.replace_selection("", EditKind::Discrete, cx);
    }

    pub(super) fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.text.selection().is_empty() {
            let caret = self.caret_offset();
            self.text
                .set_selection_for_edit(caret..self.next_word_boundary(caret));
        }
        self.replace_selection("", EditKind::Discrete, cx);
    }

    pub(super) fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1.0, false, cx);
    }

    pub(super) fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1.0, false, cx);
    }

    pub(super) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1.0, true, cx);
    }

    pub(super) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1.0, true, cx);
    }

    pub(super) fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Some((start, _)) = self.row_bounds(self.caret_offset()) {
            self.move_to(start, cx);
        }
    }

    pub(super) fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Some((_, end)) = self.row_bounds(self.caret_offset()) {
            self.move_to(end, cx);
        }
    }

    pub(super) fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Some((start, _)) = self.row_bounds(self.caret_offset()) {
            self.select_to(start, cx);
        }
    }

    pub(super) fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        if let Some((_, end)) = self.row_bounds(self.caret_offset()) {
            self.select_to(end, cx);
        }
    }

    pub(super) fn document_start(
        &mut self,
        _: &DocumentStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferred_x = None;
        self.move_to(0, cx);
    }

    pub(super) fn document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.move_to(self.text.content().len(), cx);
    }

    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.text.select_all();
        self.caret_moved(cx);
    }

    pub(super) fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.selection().is_empty() {
            let caret = self.caret_offset();
            self.text
                .set_selection_for_edit(self.previous_boundary(caret)..caret);
        }
        self.replace_selection("", EditKind::DeleteBackward, cx);
    }

    pub(super) fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.selection().is_empty() {
            let caret = self.caret_offset();
            self.text
                .set_selection_for_edit(caret..self.next_boundary(caret));
        }
        self.replace_selection("", EditKind::DeleteForward, cx);
    }

    pub(super) fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.text.selection().is_empty()
            && let Some((start, _)) = self.row_bounds(self.caret_offset())
        {
            self.text.set_selection_for_edit(start..self.caret_offset());
        }
        self.replace_selection("", EditKind::Discrete, cx);
    }

    pub(super) fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        self.replace_selection("\n", EditKind::Discrete, cx);
    }

    pub(super) fn value_at(&self, index: usize) -> Option<&str> {
        match self.displayed.get(index)?.as_ref()? {
            LineOutput::Value { string, .. } if !string.is_empty() => Some(string),
            _ => None,
        }
    }

    pub(super) fn copy_result(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(value) = self.value_at(index) else {
            return;
        };

        cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));
        self.copy_flash = Some(CopyFlash {
            result: index,
            at: Instant::now(),
        });
        self.toast.show("Copied!");
        cx.notify();
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.hovered_result
            && self.pointer_leads
            && self.value_at(index).is_some()
        {
            self.copy_result(index, cx);
            return;
        }

        if let Some(text) = self.text.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        }
    }

    pub(super) fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.text.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
            self.replace_selection("", EditKind::Discrete, cx);
        }
    }

    pub(super) fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_selection(&text, EditKind::Discrete, cx);
        }
    }

    pub(super) fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }
}
