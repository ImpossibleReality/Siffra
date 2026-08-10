mod actions;
mod diagnostics;
mod input_element;
mod interaction;
pub mod layout;
mod render;
pub(crate) mod result;

use crate::calculation::{Calculation, LineOutput};
use crate::text::{EditKind, TextBuffer};
use crate::ui::theme;
use crate::ui::toast::Toast;
use interaction::{CopyFlash, Drag, Fade, Scrollbar};
use layout::{Fonts, Layout};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub use actions::{
    DocumentEnd, DocumentStart, Down, Newline, SelectDown, SelectUp, ShowCharacterPalette, Up,
};

use gpui::{
    App, Bounds, Context, EntityInputHandler, EventEmitter, FocusHandle, Focusable, Pixels, Point,
    ScrollHandle, Task, UTF16Selection, Window, point, px,
};

/// Emitted after the document changes.
pub struct ContentChanged;

pub struct Editor {
    focus_handle: FocusHandle,
    pub(crate) text: TextBuffer,
    /// Desired x-coordinate for vertical caret movement.
    preferred_x: Option<Pixels>,
    drag: Option<Drag>,
    caret_visible: bool,
    blink: Option<Task<()>>,
    calculation: Calculation,
    outputs: Vec<Option<LineOutput>>,
    /// Results currently visible after diagnostic suppression.
    displayed: Vec<Option<LineOutput>>,
    previous_lines: Vec<String>,
    line_edited_at: Vec<Option<Instant>>,
    diagnostic_timer: Option<Task<()>>,
    fonts: Fonts,
    output_fraction: f32,
    dragging_separator: bool,
    separator_hovered: bool,
    separator_fade: Fade,
    hovered_result: Option<usize>,
    /// Whether pointer activity has priority for copy.
    pointer_leads: bool,
    pressed_result: Option<usize>,
    /// Index of the open result popover, if any.
    open_result: Option<usize>,
    /// Prevents a handled press from closing the popover.
    press_handled: bool,
    copy_flash: Option<CopyFlash>,
    toast: Toast,
    text_bounds: Option<Bounds<Pixels>>,
    layout: Option<Rc<Layout>>,
    scroll: ScrollHandle,
    autoscroll: bool,
    /// Scrollbar state, or `None` when content fits.
    scrollbar: Option<Scrollbar>,
    scrollbar_grab: Option<Pixels>,
}

impl EventEmitter<ContentChanged> for Editor {}

impl Editor {
    pub fn new(content: impl Into<String>, fonts: Fonts, cx: &mut Context<Self>) -> Self {
        let content = content.into();

        let mut editor = Self {
            focus_handle: cx.focus_handle(),
            text: TextBuffer::new(content),
            preferred_x: None,
            drag: None,
            caret_visible: true,
            blink: None,
            calculation: Calculation::default(),
            outputs: Vec::new(),
            displayed: Vec::new(),
            previous_lines: Vec::new(),
            line_edited_at: Vec::new(),
            diagnostic_timer: None,
            fonts,
            output_fraction: theme::OUTPUT_FRACTION,
            dragging_separator: false,
            separator_hovered: false,
            separator_fade: Fade::new(0.0),
            hovered_result: None,
            pointer_leads: false,
            pressed_result: None,
            open_result: None,
            press_handled: false,
            copy_flash: None,
            toast: Toast::new(),
            text_bounds: None,
            layout: None,
            scroll: ScrollHandle::new(),
            autoscroll: false,
            scrollbar: None,
            scrollbar_grab: None,
        };

        editor.recalculate();
        editor.refresh_diagnostics(cx);
        editor
    }

    pub fn show_toast(&mut self, message: impl Into<gpui::SharedString>, cx: &mut Context<Self>) {
        self.toast.show(message);
        cx.notify();
    }

    pub fn name_editing_started(&mut self, cx: &mut Context<Self>) {
        self.pointer_leads = false;
        cx.notify();
    }

    fn caret_offset(&self) -> usize {
        self.text.caret_offset()
    }

    pub fn content(&self) -> &str {
        self.text.content()
    }

    /// Returns the caret as a UTF-16 offset for platform text input.
    pub fn cursor(&self) -> usize {
        self.text.offset_to_utf16(self.caret_offset())
    }

    /// Replaces the document and resets transient editor state.
    pub fn set_document(&mut self, content: &str, cursor: usize, cx: &mut Context<Self>) {
        self.text.reset(content);
        let caret = self.text.offset_from_utf16(cursor);
        self.text.move_to(caret);
        self.preferred_x = None;
        self.drag = None;
        self.open_result = None;
        self.hovered_result = None;
        self.pressed_result = None;
        self.copy_flash = None;
        self.displayed = Vec::new();
        self.previous_lines = Vec::new();
        self.line_edited_at = Vec::new();
        self.diagnostic_timer = None;
        self.layout = None;
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        self.recalculate();
        self.line_edited_at = Vec::new();
        self.caret_moved(cx);
    }

    pub fn focus(&mut self, window: &mut Window) {
        self.focus_handle.focus(window);
    }

    pub fn focus_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window);
        self.move_to(self.text.content().len(), cx);
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.text.move_to(offset);
        self.caret_moved(cx);
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.text.select_to(offset);
        self.caret_moved(cx);
    }

    fn caret_moved(&mut self, cx: &mut Context<Self>) {
        self.pointer_leads = false;
        self.caret_visible = true;
        self.autoscroll = true;
        self.refresh_diagnostics(cx);
        self.blink = Some(cx.spawn(async move |editor, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs_f32(theme::CARET_BLINK_PERIOD / 2.0))
                    .await;
                let updated = editor.update(cx, |editor, cx| {
                    editor.caret_visible = !editor.caret_visible;
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.text.previous_boundary(offset)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.text.next_boundary(offset)
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        self.text.previous_word_boundary(offset)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        self.text.next_word_boundary(offset)
    }

    fn word_range_at(&self, offset: usize) -> Range<usize> {
        self.text.word_range_at(offset)
    }

    fn line_range_at(&self, offset: usize) -> Range<usize> {
        let Some(layout) = self.layout.as_ref() else {
            return offset..offset;
        };
        let line = &layout.lines[layout.line_index_for_offset(offset)];
        line.start..line.start + line.len
    }

    fn row_bounds(&self, offset: usize) -> Option<(usize, usize)> {
        let layout = self.layout.as_ref()?;
        let position = layout.position_for_offset(offset)?;
        Some((
            layout.offset_for_position(point(px(0.0), position.y)),
            layout.offset_for_position(point(px(f32::MAX / 2.0), position.y)),
        ))
    }

    fn move_vertically(&mut self, rows: f32, extend: bool, cx: &mut Context<Self>) {
        let Some(layout) = self.layout.clone() else {
            return;
        };
        let Some(position) = layout.position_for_offset(self.caret_offset()) else {
            return;
        };

        let x = self.preferred_x.unwrap_or(position.x);
        let line_height = layout.metrics.line_height;
        let y = position.y + line_height * rows + line_height / 2.0;
        let offset = layout.offset_for_position(point(x, y));

        if extend {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
        self.preferred_x = Some(x);
    }

    fn replace_selection(&mut self, text: &str, kind: EditKind, cx: &mut Context<Self>) {
        self.text.replace_selection(text, kind);
        self.text_changed(cx);
    }

    fn text_changed(&mut self, cx: &mut Context<Self>) {
        self.preferred_x = None;
        self.recalculate();
        self.caret_moved(cx);
        cx.emit(ContentChanged);
    }

    fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(layout)) = (self.text_bounds, self.layout.as_ref()) else {
            return 0;
        };
        layout.offset_for_position(position - bounds.origin)
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Editor {
    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text.offset_to_utf16(offset)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.text.range_from_utf16(range)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.text.range_to_utf16(range)
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.text.content().get(range)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(self.text.selection()),
            reversed: self.text.selection_reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.text
            .marked_range()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.text.unmark_text();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.text.replace_text_in_range(range_utf16, new_text);
        self.text_changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.text
            .replace_and_mark_text_in_range(range_utf16, new_text, new_selected_range_utf16);
        self.text_changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start = layout.position_for_offset(range.start)?;
        let end = layout.position_for_offset(range.end)?;
        Some(Bounds::from_corners(
            element_bounds.origin + start,
            element_bounds.origin + point(end.x, end.y + layout.metrics.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.offset_for_position(point)))
    }
}

#[cfg(test)]
mod tests {
    use super::diagnostics::carry_edit_times;

    fn lines(text: &str) -> Vec<String> {
        text.split('\n').map(str::to_string).collect()
    }

    fn carry(previous: &str, times: &[u32], next: &str) -> Vec<Option<u32>> {
        let times: Vec<Option<u32>> = times.iter().map(|time| Some(*time)).collect();
        carry_edit_times(&lines(previous), &times, &lines(next), 2)
    }

    #[test]
    fn typing_on_one_line_leaves_its_neighbours_alone() {
        assert_eq!(
            carry("a\nb\nc", &[1, 1, 1], "a\nB\nc"),
            [Some(1), Some(2), Some(1)]
        );
    }

    #[test]
    fn a_change_that_changes_nothing_resets_no_timers() {
        assert_eq!(
            carry("a\nb\nc", &[1, 1, 1], "a\nb\nc"),
            [Some(1), Some(1), Some(1)]
        );
    }

    #[test]
    fn inserting_a_line_costs_only_that_line() {
        assert_eq!(
            carry("a\nb", &[1, 1], "a\n\nb"),
            [Some(1), Some(2), Some(1)]
        );
    }

    #[test]
    fn deleting_a_line_leaves_the_survivors_alone() {
        assert_eq!(carry("a\nb\nc", &[1, 1, 1], "a\nc"), [Some(1), Some(1)]);
    }

    #[test]
    fn typing_at_the_end_leaves_the_lines_above_alone() {
        assert_eq!(
            carry("a\nb", &[1, 1], "a\nb\nc"),
            [Some(1), Some(1), Some(2)]
        );
    }

    #[test]
    fn every_line_of_a_freshly_loaded_calculation_counts_as_typed() {
        assert_eq!(carry("", &[], "a\nb"), [Some(2), Some(2)]);
    }
}
