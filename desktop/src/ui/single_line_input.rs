//! A reusable single-line field with native text input and IME support.
//!
//! Features own what the value means and what to do with it. The field emits a
//! change as soon as its text mutates, so a search can debounce matching and a
//! document title can save or validate independently of text entry.
//!
//! What the box around the text looks like is the owner's business too: the
//! field draws the text, the selection and the caret, and hands its own element
//! to a closure the owner supplied to be dressed. Everything the field needs to
//! know to place a caret — its padding, how tall the caret stands, whether the
//! text is centred — is data it is built with.

use crate::text_input::{
    Backspace, Copy, Cut, Delete, DeleteToLineStart, DeleteWordLeft, DeleteWordRight, EditKind,
    End, Home, Left, Paste, Redo, Right, SelectAll, SelectEnd, SelectHome, SelectLeft, SelectRight,
    SelectWordLeft, SelectWordRight, TextBuffer, Undo, WordLeft, WordRight,
};
use crate::theme;
use gpui::{
    App, Bounds, ClipboardItem, Context, Corners, Div, ElementInputHandler, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, HighlightStyle, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, Rgba,
    ShapedLine, SharedString, Stateful, Styled, StyledText, TextRun, UTF16Selection, Window,
    canvas, div, point, prelude::FluentBuilder, px, quad, size,
};
use std::ops::Range;

/// Emitted after a user edit changes the field's value.
pub struct ValueChanged;

/// How the field is drawn, in the measurements the caret and the selection are
/// placed against.
#[derive(Clone, Copy)]
pub struct FieldMetrics {
    pub padding_x: Pixels,
    pub padding_y: Pixels,
    /// The caret's height: the text's own line box, not the whole field.
    pub caret_height: Pixels,
    /// Background color for selected text.
    pub selection: Rgba,
    pub selection_text: Option<Rgba>,
    /// Color shown while the field is empty.
    pub placeholder: Rgba,
    /// Whether text and its caret are centered in the field.
    pub centered: bool,
}

/// What the owner's chrome needs to know to dress the field.
#[derive(Clone, Copy)]
pub struct FieldState {
    pub focused: bool,
    pub empty: bool,
}

type Chrome = Box<dyn Fn(Stateful<Div>, FieldState) -> Stateful<Div>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectionGranularity {
    Character,
    Word,
    Line,
}

struct SelectionDrag {
    granularity: SelectionGranularity,
    anchor: Range<usize>,
}

pub struct SingleLineInput {
    id: SharedString,
    focus_handle: FocusHandle,
    placeholder: SharedString,
    text: TextBuffer,
    metrics: FieldMetrics,
    chrome: Chrome,
    /// Set while the field is only there to be read. A field that cannot be
    /// typed into is not somewhere to put a caret either, so a press on one
    /// passes straight through to whatever the owner wrapped it in.
    read_only: bool,
    /// A ceiling the owner recomputes as the window resizes, which is why it is
    /// held here rather than baked into the chrome.
    max_width: Option<Pixels>,
    drag: Option<SelectionDrag>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    focus_on_next_render: bool,
}

impl EventEmitter<ValueChanged> for SingleLineInput {}

impl SingleLineInput {
    pub fn new(
        id: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
        metrics: FieldMetrics,
        chrome: impl Fn(Stateful<Div>, FieldState) -> Stateful<Div> + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            id: id.into(),
            focus_handle: cx.focus_handle(),
            placeholder: placeholder.into(),
            text: TextBuffer::new(""),
            metrics,
            chrome: Box::new(chrome),
            read_only: false,
            max_width: None,
            drag: None,
            last_layout: None,
            last_bounds: None,
            focus_on_next_render: false,
        }
    }

    pub fn value(&self) -> &str {
        self.text.content()
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    /// Replaces the value without emitting [`ValueChanged`]. This is intended
    /// for owner-driven synchronization rather than a user edit.
    pub fn set_value(&mut self, value: impl Into<String>, cx: &mut Context<Self>) {
        self.text.reset(single_line(value.into()));
        self.drag = None;
        self.last_layout = None;
        cx.notify();
    }

    /// Stops the field taking a caret, so a press on it is the owner's to act on.
    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        if self.read_only != read_only {
            self.read_only = read_only;
            self.drag = None;
            cx.notify();
        }
    }

    /// The widest the field may be drawn. Set from the owner's own layout, which
    /// is why nothing is redrawn here: the owner is already mid-render.
    pub fn set_max_width(&mut self, max_width: Option<Pixels>) {
        self.max_width = max_width;
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.text.select_all();
        cx.notify();
    }

    pub fn request_focus(&mut self, cx: &mut Context<Self>) {
        self.focus_on_next_render = true;
        cx.notify();
    }

    fn emit_changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(ValueChanged);
        cx.notify();
    }

    /// How far the text is held off the left edge of its box, which is nothing
    /// unless the field centres text that is narrower than the space for it.
    fn alignment_offset(&self, line: &ShapedLine, bounds: Bounds<Pixels>) -> Pixels {
        if !self.metrics.centered {
            return px(0.0);
        }
        let inner = (bounds.size.width - self.metrics.padding_x * 2.0).max(px(0.0));
        ((inner - line.width) / 2.0).max(px(0.0))
    }

    fn index_for_position(&self, position: gpui::Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (&self.last_bounds, &self.last_layout) else {
            return self.text.content().len();
        };
        if position.x <= bounds.left() {
            return 0;
        }
        if position.x >= bounds.right() {
            return self.text.content().len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn drag_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_ref() else {
            return;
        };
        let reach = match drag.granularity {
            SelectionGranularity::Character => offset..offset,
            SelectionGranularity::Word => self.text.word_range_at(offset),
            SelectionGranularity::Line => 0..self.text.content().len(),
        };
        let anchor = drag.anchor.clone();
        self.text.set_selection(
            anchor.start.min(reach.start)..anchor.end.max(reach.end),
            reach.start < anchor.start,
        );
        cx.notify();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }

        self.focus_handle.focus(window);
        self.text.close_history_group();
        let offset = self.index_for_position(event.position);
        let (granularity, anchor) = match event.click_count {
            0 | 1 if event.modifiers.shift => {
                let fixed = if self.text.selection_reversed() {
                    self.text.selection().end
                } else {
                    self.text.selection().start
                };
                (SelectionGranularity::Character, fixed..fixed)
            }
            0 | 1 => (SelectionGranularity::Character, offset..offset),
            2 => (SelectionGranularity::Word, self.text.word_range_at(offset)),
            _ => (SelectionGranularity::Line, 0..self.text.content().len()),
        };
        self.drag = Some(SelectionDrag {
            granularity,
            anchor,
        });
        self.drag_to(offset, cx);
        cx.stop_propagation();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            self.drag_to(self.index_for_position(event.position), cx);
            cx.stop_propagation();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            cx.notify();
            cx.stop_propagation();
        }
    }

    fn replace_selection(&mut self, text: &str, kind: EditKind, cx: &mut Context<Self>) {
        self.text.replace_selection(text, kind);
        self.emit_changed(cx);
    }

    /// Moves the caret, or drags the loose end of a selection to the same place.
    fn go_to(&mut self, destination: usize, selecting: bool, cx: &mut Context<Self>) {
        if selecting {
            self.text.select_to(destination);
        } else {
            self.text.move_to(destination);
        }
        cx.notify();
    }

    /// Deletes back to `start`, or deletes the selection when there is one —
    /// which is what every backward delete does, differing only in how far it
    /// would reach on its own and whether it groups with the last one.
    fn delete_back_to(&mut self, start: usize, kind: EditKind, cx: &mut Context<Self>) {
        if self.text.selection().is_empty() {
            self.text
                .set_selection_for_edit(start..self.text.caret_offset());
        }
        if !self.text.selection().is_empty() {
            self.replace_selection("", kind, cx);
        }
    }

    fn delete_forward_to(&mut self, end: usize, kind: EditKind, cx: &mut Context<Self>) {
        if self.text.selection().is_empty() {
            self.text
                .set_selection_for_edit(self.text.caret_offset()..end);
        }
        if !self.text.selection().is_empty() {
            self.replace_selection("", kind, cx);
        }
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let start = self.text.previous_boundary(self.text.caret_offset());
        self.delete_back_to(start, EditKind::DeleteBackward, cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.text.next_boundary(self.text.caret_offset());
        self.delete_forward_to(end, EditKind::DeleteForward, cx);
    }

    fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_back_to(0, EditKind::Discrete, cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let start = self.text.previous_word_boundary(self.text.caret_offset());
        self.delete_back_to(start, EditKind::Discrete, cx);
    }

    fn delete_word_right(&mut self, _: &DeleteWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.text.next_word_boundary(self.text.caret_offset());
        self.delete_forward_to(end, EditKind::Discrete, cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        // Leaving a selection by the left arrow lands on its near edge rather
        // than a step further back.
        let selection = self.text.selection().clone();
        let destination = if selection.is_empty() {
            self.text.previous_boundary(self.text.caret_offset())
        } else {
            selection.start
        };
        self.go_to(destination, false, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let selection = self.text.selection().clone();
        let destination = if selection.is_empty() {
            self.text.next_boundary(self.text.caret_offset())
        } else {
            selection.end
        };
        self.go_to(destination, false, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        let destination = self.text.previous_boundary(self.text.caret_offset());
        self.go_to(destination, true, cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        let destination = self.text.next_boundary(self.text.caret_offset());
        self.go_to(destination, true, cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let destination = self.text.previous_word_boundary(self.text.caret_offset());
        self.go_to(destination, false, cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        let destination = self.text.next_word_boundary(self.text.caret_offset());
        self.go_to(destination, false, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let destination = self.text.previous_word_boundary(self.text.caret_offset());
        self.go_to(destination, true, cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        let destination = self.text.next_word_boundary(self.text.caret_offset());
        self.go_to(destination, true, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.go_to(0, false, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.go_to(self.text.content().len(), false, cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.go_to(0, true, cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.go_to(self.text.content().len(), true, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.text.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.text.selected_text().map(str::to_string) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.replace_selection("", EditKind::Discrete, cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        // Whatever was copied arrives on one line here, however many it had.
        self.replace_selection(&text.replace(['\r', '\n'], " "), EditKind::Discrete, cx);
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.undo() {
            self.emit_changed(cx);
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.text.redo() {
            self.emit_changed(cx);
        }
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.text.range_from_utf16(range)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.text.range_to_utf16(range)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text.offset_to_utf16(offset)
    }
}

impl Focusable for SingleLineInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SingleLineInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_on_next_render) {
            self.focus_handle.focus(window);
        }

        let state = FieldState {
            focused: self.focus_handle.is_focused(window),
            empty: self.text.content().is_empty(),
        };
        let selection = self.text.selection().clone();
        let content = if state.empty {
            div()
                .text_color(self.metrics.placeholder)
                .child(self.placeholder.clone())
        } else if selection.is_empty() {
            div().child(self.text.content().to_string())
        } else {
            div().child(
                StyledText::new(self.text.content().to_string()).with_highlights(Some((
                    selection,
                    HighlightStyle {
                        background_color: Some(self.metrics.selection.into()),
                        color: self.metrics.selection_text.map(Into::into),
                        ..Default::default()
                    },
                ))),
            )
        };
        let input = cx.entity();
        let metrics = self.metrics;

        let field = div()
            .id(self.id.clone())
            .relative()
            .px(metrics.padding_x)
            .py(metrics.padding_y)
            .flex()
            .items_center()
            // Center text in the field box: a field that shrinks to fit its
            // text cannot also provide that text's centering width.
            .when(metrics.centered, |field| field.justify_center())
            .when_some(self.max_width, |field, max| field.max_w(max))
            .track_focus(&self.focus_handle)
            // Names the field to the keymap, so `main` can bind the editing
            // commands here without them reaching the calculation, and can bind
            // them there without them reaching a field.
            .key_context("SiffraInput")
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_to_line_start))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(|input, _: &SelectAll, _, cx| input.select_all(cx)))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up));

        (self.chrome)(field, state).child(content).child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| {
                    let (focus_handle, value, selection, caret) = {
                        let input = input.read(cx);
                        (
                            input.focus_handle.clone(),
                            SharedString::from(input.text.content().to_string()),
                            input.text.selection().clone(),
                            input.text.caret_offset(),
                        )
                    };
                    window.handle_input(
                        &focus_handle,
                        ElementInputHandler::new(bounds, input.clone()),
                        cx,
                    );

                    let style = window.text_style();
                    let runs = (!value.is_empty())
                        .then(|| TextRun {
                            len: value.len(),
                            font: style.font(),
                            color: style.color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        })
                        .into_iter()
                        .collect::<Vec<_>>();
                    let font_size = style.font_size.to_pixels(window.rem_size());
                    let line = window
                        .text_system()
                        .shape_line(value, font_size, &runs, None);
                    let offset = input.read(cx).alignment_offset(&line, bounds);
                    let text_bounds = Bounds::new(
                        point(
                            bounds.origin.x + metrics.padding_x + offset,
                            bounds.origin.y,
                        ),
                        size(line.width, bounds.size.height),
                    );

                    // A native caret occupies an insertion coordinate; it
                    // is not an inline box between two separately shaped
                    // pieces of text. Painting it over one shaped line
                    // preserves kerning and does not push either neighbor.
                    if focus_handle.is_focused(window) && selection.is_empty() {
                        window.paint_quad(quad(
                            Bounds::new(
                                point(
                                    text_bounds.left() + line.x_for_index(caret),
                                    bounds.top()
                                        + (bounds.size.height - metrics.caret_height) / 2.0,
                                ),
                                size(px(theme::CARET_WIDTH), metrics.caret_height),
                            ),
                            Corners::all(px(theme::CARET_WIDTH / 2.0)),
                            theme::hsla(theme::caret()),
                            px(0.0),
                            gpui::transparent_black(),
                            gpui::BorderStyle::default(),
                        ));
                    }

                    input.update(cx, |input, _| {
                        input.last_layout = Some(line);
                        input.last_bounds = Some(text_bounds);
                    });
                },
            )
            .absolute()
            .inset_0(),
        )
    }
}

impl EntityInputHandler for SingleLineInput {
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
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.text
            .replace_text_in_range(range_utf16, &text.replace(['\r', '\n'], " "));
        self.emit_changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selected_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        self.text.replace_and_mark_text_in_range(
            range_utf16,
            &text.replace(['\r', '\n'], " "),
            selected_utf16,
        );
        self.emit_changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let bounds = self.last_bounds?;
        let range = self.text.range_from_utf16(&range_utf16);
        let left = bounds.left() + line.x_for_index(range.start);
        let right = bounds.left() + line.x_for_index(range.end);
        Some(Bounds::from_corners(
            point(left, bounds.top()),
            point(right, bounds.top() + self.metrics.caret_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.offset_to_utf16(self.index_for_position(point)))
    }
}

fn single_line(value: String) -> String {
    value.replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::single_line;

    #[test]
    fn programmatic_values_cannot_introduce_another_line() {
        assert_eq!(
            single_line("first\rsecond\nthird".to_string()),
            "first second third"
        );
    }
}
