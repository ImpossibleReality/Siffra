mod actions;
mod diagnostics;
mod input_element;
mod interaction;
pub mod layout;
mod render;
pub(crate) mod result;

use crate::calculation::{Calculation, LineOutput};
use crate::text::{EditKind, TextBuffer};
use crate::ui::popover::{below_right, popover_button};
use crate::ui::theme;
use crate::ui::toast::Toast;
use input_element::InputElement;
use interaction::{CopyFlash, Drag, Fade, Granularity, Scrollbar};
use layout::{Fonts, Layout, Metrics};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub use actions::{
    DocumentEnd, DocumentStart, Down, Newline, SelectDown, SelectUp, ShowCharacterPalette, Up,
};

use gpui::{
    App, Bounds, ClickEvent, Context, CursorStyle, Div, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Task, UTF16Selection, Window,
    div, point, prelude::FluentBuilder, px,
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

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.preferred_x = None;
        self.text.close_history_group();
        let offset = self.offset_for_position(event.position);

        let (granularity, anchor) = match event.click_count {
            0 | 1 if event.modifiers.shift => {
                let fixed = if self.text.selection_reversed() {
                    self.text.selection().end
                } else {
                    self.text.selection().start
                };
                (Granularity::Character, fixed..fixed)
            }
            0 | 1 => (Granularity::Character, offset..offset),
            2 => (Granularity::Word, self.word_range_at(offset)),
            _ => (Granularity::Line, self.line_range_at(offset)),
        };

        self.drag = Some(Drag {
            granularity,
            anchor,
        });
        self.drag_to(offset, cx);
    }

    fn drag_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_ref() else {
            return;
        };

        let reach = match drag.granularity {
            Granularity::Character => offset..offset,
            Granularity::Word => self.word_range_at(offset),
            Granularity::Line => self.line_range_at(offset),
        };
        let anchor = drag.anchor.clone();

        let selection = anchor.start.min(reach.start)..anchor.end.max(reach.end);
        self.text
            .set_selection(selection, reach.start < anchor.start);
        self.caret_moved(cx);
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.pointer_leads = true;

        if self.drag.is_some() {
            let offset = self.offset_for_position(event.position);
            self.drag_to(offset, cx);
        }

        if self.dragging_separator {
            let width = f32::from(
                self.layout
                    .as_ref()
                    .map_or(px(1.0), |l| l.metrics.window_width),
            );
            self.output_fraction = (1.0 - f32::from(event.position.x) / width).clamp(0.0, 1.0);
            cx.notify();
        }

        if let (Some(grab), Some(bar)) = (self.scrollbar_grab, self.scrollbar) {
            self.scroll_to_thumb_top(event.position.y - grab, bar);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some()
            || self.dragging_separator
            || self.scrollbar_grab.is_some()
            || self.pressed_result.is_some()
        {
            self.drag = None;
            self.dragging_separator = false;
            self.scrollbar_grab = None;
            self.pressed_result = None;
            cx.notify();
        }
    }

    fn scroll_caret_into_view(&self, layout: &Layout, view_height: Pixels, content_height: Pixels) {
        let Some(position) = layout.position_for_offset(self.caret_offset()) else {
            return;
        };
        if view_height <= px(0.0) {
            return;
        }

        let margin = px(theme::GUTTER);
        let top = px(theme::GUTTER) + position.y;
        let bottom = top + layout.metrics.line_height;

        let mut offset = self.scroll.offset();
        let view_top = -offset.y;
        if top - margin < view_top {
            offset.y = -(top - margin);
        } else if bottom + margin > view_top + view_height {
            offset.y = view_height - (bottom + margin);
        } else {
            return;
        }

        let scroll_max = (content_height - view_height).max(px(0.0));
        offset.y = offset.y.clamp(-scroll_max, px(0.0));
        self.scroll.set_offset(offset);
    }

    fn scroll_to_thumb_top(&self, top: Pixels, bar: Scrollbar) {
        let travel = bar.track_height - bar.thumb_height;
        let progress = if travel > px(0.0) {
            (f32::from(top - bar.track_top) / f32::from(travel)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut offset = self.scroll.offset();
        offset.y = -bar.scroll_max * progress;
        self.scroll.set_offset(offset);
    }

    fn preserve_scroll_position_for_font_resize(
        &self,
        content_height: Pixels,
        view_height: Pixels,
    ) {
        let Some(previous) = self.scrollbar else {
            return;
        };

        let progress =
            (f32::from(-self.scroll.offset().y) / f32::from(previous.scroll_max)).clamp(0.0, 1.0);
        let scroll_max = (content_height - view_height).max(px(0.0));
        let mut offset = self.scroll.offset();
        offset.y = -scroll_max * progress;
        self.scroll.set_offset(offset);
    }

    fn measure_scrollbar(&mut self, view_height: Pixels, content_height: Pixels) {
        let scroll_max = content_height - view_height;
        if scroll_max <= px(0.5) || view_height <= px(0.0) {
            self.scrollbar = None;
            return;
        }

        let inset = px(theme::SCROLLBAR_INSET);
        let foot = inset + px(theme::corner_clearance(theme::SCROLLBAR_INSET));
        let track_height = (view_height - inset - foot).max(px(0.0));
        let thumb_height = (track_height * (f32::from(view_height) / f32::from(content_height)))
            .max(px(theme::SCROLLBAR_MIN_THUMB))
            .min(track_height);

        let progress = (f32::from(-self.scroll.offset().y) / f32::from(scroll_max)).clamp(0.0, 1.0);
        let track_top = px(theme::NAV_HEIGHT) + inset;

        self.scrollbar = Some(Scrollbar {
            track_top,
            track_height,
            thumb_top: track_top + (track_height - thumb_height) * progress,
            thumb_height,
            scroll_max,
        });
    }

    fn render_scrollbar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let bar = self.scrollbar?;

        Some(
            div()
                .absolute()
                .top(bar.thumb_top)
                .right(px(theme::SCROLLBAR_INSET))
                .w(px(theme::SCROLLBAR_WIDTH))
                .h(bar.thumb_height)
                .child(
                    div()
                        .id("scrollbar-thumb")
                        .size_full()
                        .rounded(px(theme::SCROLLBAR_WIDTH / 2.0))
                        .bg(theme::scrollbar_thumb())
                        .cursor(CursorStyle::Arrow)
                        .hover(|style| style.bg(theme::scrollbar_thumb_hover()))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                                editor.scrollbar_grab = Some(event.position.y - bar.thumb_top);
                                cx.notify();
                            }),
                        ),
                ),
        )
    }

    fn render_result(
        &self,
        index: usize,
        output: Option<&LineOutput>,
        height: Pixels,
        budget: usize,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Div {
        let row = div()
            .h(height)
            .flex()
            .flex_row()
            .items_center()
            .justify_end();

        let Some(output) = output else {
            return row;
        };

        let copied = self.copy_flash.filter(|flash| flash.result == index);

        let interaction = if self.open_result == Some(index) {
            result::Interaction::Open
        } else if self.pressed_result == Some(index) || copied.is_some_and(|flash| flash.lit()) {
            result::Interaction::Pressed
        } else if self.hovered_result == Some(index) {
            result::Interaction::Hovered
        } else {
            result::Interaction::Idle
        };

        row.child(
            div()
                .id(("result", index))
                .cursor(CursorStyle::Arrow)
                .on_hover(cx.listener(move |editor, hovered: &bool, _, cx| {
                    if *hovered {
                        editor.hovered_result = Some(index);
                    } else if editor.hovered_result == Some(index) {
                        editor.hovered_result = None;
                    }
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                        editor.pressed_result = Some(index);
                        editor.press_handled = true;
                        cx.notify();
                    }),
                )
                .on_click(cx.listener(move |editor, _: &ClickEvent, _, cx| {
                    editor.open_result = if editor.open_result == Some(index) {
                        None
                    } else {
                        Some(index)
                    };
                    cx.notify();
                }))
                .child(result::ResultElement::new(
                    output,
                    budget,
                    interaction,
                    metrics,
                    &self.fonts,
                    metrics.output_width - px(2.0 * theme::GUTTER),
                    copied.map_or(1.0, |flash| flash.scale()),
                )),
        )
    }

    fn render_popover(&self, layout: &Layout, cx: &mut Context<Self>) -> Option<Div> {
        let index = self.open_result?;
        let output = self.displayed.get(index)?.as_ref()?;

        let result_height = px(theme::ROOT_LINE_HEIGHT + 2.0 * theme::OUTPUT_PADDING_Y);
        let row_height = layout.lines.get(index)?.height;
        let row_top =
            px(theme::NAV_HEIGHT + theme::GUTTER) + layout.line_top(index) + self.scroll.offset().y;
        let bottom = row_top + (row_height - result_height) / 2.0 + result_height;

        let panel = below_right(bottom, px(theme::GUTTER))
            .flex()
            .flex_col()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, _, _| {
                    editor.press_handled = true;
                }),
            );

        Some(match output {
            LineOutput::Value {
                string, repeating, ..
            } => {
                let copied = self
                    .copy_flash
                    .is_some_and(|flash| flash.result == index && flash.lit());

                panel
                    .child(
                        div()
                            .mb(px(theme::POPOVER_VALUE_GAP))
                            .child(result::ValueElement::new(
                                string,
                                *repeating,
                                &self.fonts,
                                px(theme::POPOVER_MAX_WIDTH - 2.0 * theme::POPOVER_PADDING),
                            )),
                    )
                    .child(
                        popover_button(
                            ("copy-result", index),
                            if copied { "Copied!" } else { "Copy" },
                        )
                        .on_click(cx.listener(
                            move |editor, _: &ClickEvent, _, cx| {
                                editor.copy_result(index, cx);
                            },
                        )),
                    )
            }
            LineOutput::Error {
                description,
                location,
                ..
            } => panel
                .child(
                    div()
                        .text_size(px(theme::FONT_SMALL))
                        .text_color(theme::popover_error_description())
                        .child(SharedString::from(description.clone().unwrap_or_default())),
                )
                .children(location.as_ref().map(|location| {
                    div()
                        .mt(px(theme::POPOVER_LOCATION_GAP))
                        .text_size(px(theme::FONT_TINY))
                        .text_color(theme::popover_error_location())
                        .child(SharedString::from(location.clone()))
                })),
        })
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

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = Metrics::new(window, &self.fonts, self.output_fraction);
        let layout = Rc::new(Layout::new(
            self.text.content(),
            self.text.selection(),
            metrics,
            &self.fonts,
            window,
        ));
        let font_size_changed = self
            .layout
            .as_ref()
            .is_some_and(|previous| previous.metrics.font_size != layout.metrics.font_size);
        self.layout = Some(layout.clone());

        self.separator_fade.set(
            if layout.any_wrapped || self.separator_hovered || self.dragging_separator {
                1.0
            } else {
                0.0
            },
        );
        if !self.separator_fade.settled() {
            window.request_animation_frame();
        }

        let viewport = window.viewport_size();
        let min_height =
            (viewport.height - px(theme::NAV_HEIGHT) - px(2.0 * theme::GUTTER)).max(px(0.0));
        let budget = metrics.output_character_budget();

        let view_height = viewport.height - px(theme::NAV_HEIGHT);
        let content_height = layout.content_height().max(min_height) + px(2.0 * theme::GUTTER);

        if font_size_changed {
            self.preserve_scroll_position_for_font_resize(content_height, view_height);
        }
        if std::mem::take(&mut self.autoscroll) {
            self.scroll_caret_into_view(&layout, view_height, content_height);
        }
        self.measure_scrollbar(view_height, content_height);

        let visible = |index: usize| -> Option<&LineOutput> {
            self.displayed.get(index).and_then(|output| output.as_ref())
        };

        let results: Vec<_> = layout
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                self.render_result(index, visible(index), line.height, budget, metrics, cx)
            })
            .collect();

        let error_spans: Vec<Option<Range<usize>>> = layout
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let Some(LineOutput::Error {
                    span: Some((start, end)),
                    ..
                }) = visible(index)
                else {
                    return None;
                };

                let text = &self.text.content()[line.start..line.start + line.len];
                let range = byte_offset(text, *start)..byte_offset(text, *end);
                (range.start < range.end).then_some(range)
            })
            .collect();

        let rulers: Vec<_> = layout
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let output = visible(index);
                let end_padding = output.map_or(px(0.0), |output| {
                    result::width(output, budget, metrics)
                        .min(metrics.output_width - px(2.0 * theme::GUTTER))
                });
                let active = metrics.rulers_visible() && output.is_some();

                let span = metrics.window_width
                    - px(2.0 * theme::GUTTER)
                    - line.ruler_start
                    - px(theme::RULER_GAP_LEFT)
                    - end_padding
                    - px(theme::RULER_GAP_RIGHT);

                div().h(line.height).flex().flex_row().items_center().child(
                    div()
                        .ml(line.ruler_start + px(theme::RULER_GAP_LEFT))
                        .w(span.max(px(0.0)))
                        .h(px(theme::RULER_THICKNESS))
                        .when(active, |this| this.bg(theme::ruler())),
                )
            })
            .collect();

        let popover = self.render_popover(&layout, cx);
        let toast = self.toast.render();

        if self.toast.animating() || self.copy_flash.is_some_and(|flash| flash.lit()) {
            window.request_animation_frame();
        }

        div()
            .size_full()
            .relative()
            .bg(theme::BACKGROUND)
            .text_color(theme::text())
            .text_size(px(theme::REM))
            .line_height(px(theme::ROOT_LINE_HEIGHT))
            .font_family(self.fonts.regular.family.clone())
            .font_weight(FontWeight::SEMIBOLD)
            .flex()
            .flex_col()
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                    if !std::mem::take(&mut editor.press_handled) {
                        editor.open_result = None;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .id("editor-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_row()
                            .w_full()
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .p(px(theme::GUTTER))
                                    .children(rulers),
                            )
                            .child(
                                div()
                                    .key_context("SiffraEditor")
                                    .track_focus(&self.focus_handle)
                                    .w(metrics.input_width)
                                    .flex_shrink_0()
                                    .overflow_hidden()
                                    .p(px(theme::GUTTER))
                                    .cursor(CursorStyle::IBeam)
                                    .on_action(cx.listener(Self::backspace))
                                    .on_action(cx.listener(Self::delete))
                                    .on_action(cx.listener(Self::delete_to_line_start))
                                    .on_action(cx.listener(Self::left))
                                    .on_action(cx.listener(Self::right))
                                    .on_action(cx.listener(Self::up))
                                    .on_action(cx.listener(Self::down))
                                    .on_action(cx.listener(Self::word_left))
                                    .on_action(cx.listener(Self::word_right))
                                    .on_action(cx.listener(Self::select_word_left))
                                    .on_action(cx.listener(Self::select_word_right))
                                    .on_action(cx.listener(Self::delete_word_left))
                                    .on_action(cx.listener(Self::delete_word_right))
                                    .on_action(cx.listener(Self::select_left))
                                    .on_action(cx.listener(Self::select_right))
                                    .on_action(cx.listener(Self::select_up))
                                    .on_action(cx.listener(Self::select_down))
                                    .on_action(cx.listener(Self::select_all))
                                    .on_action(cx.listener(Self::home))
                                    .on_action(cx.listener(Self::end))
                                    .on_action(cx.listener(Self::select_home))
                                    .on_action(cx.listener(Self::select_end))
                                    .on_action(cx.listener(Self::document_start))
                                    .on_action(cx.listener(Self::document_end))
                                    .on_action(cx.listener(Self::newline))
                                    .on_action(cx.listener(Self::copy))
                                    .on_action(cx.listener(Self::cut))
                                    .on_action(cx.listener(Self::paste))
                                    .on_action(cx.listener(Self::undo))
                                    .on_action(cx.listener(Self::redo))
                                    .on_action(cx.listener(Self::show_character_palette))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(Self::on_mouse_down),
                                    )
                                    .child(InputElement::new(
                                        cx.entity(),
                                        layout.clone(),
                                        min_height,
                                        error_spans,
                                    )),
                            )
                            .child(self.render_separator(cx))
                            .child(
                                div()
                                    .w(metrics.output_width)
                                    .flex_shrink_0()
                                    .p(px(theme::GUTTER))
                                    .children(results),
                            ),
                    ),
            )
            .children(self.render_scrollbar(cx))
            .children(popover)
            .children(toast)
    }
}

/// Converts a character index to a UTF-8 byte offset.
fn byte_offset(text: &str, characters: usize) -> usize {
    text.char_indices()
        .nth(characters)
        .map_or(text.len(), |(offset, _)| offset)
}

impl Editor {
    fn render_separator(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let grab = px(theme::SEPARATOR_GRAB_PADDING);

        div()
            .w(px(theme::SEPARATOR_WIDTH))
            .flex_shrink_0()
            .relative()
            .child(
                div()
                    .id("separator")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(-(grab + px(theme::SEPARATOR_WIDTH / 2.0)))
                    .w(grab * 2.0 + px(theme::SEPARATOR_WIDTH))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .cursor(CursorStyle::ResizeLeftRight)
                    .opacity(self.separator_fade.value())
                    .on_hover(cx.listener(|editor, hovered: &bool, _, cx| {
                        editor.separator_hovered = *hovered;
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                            editor.dragging_separator = true;
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .w(px(theme::SEPARATOR_WIDTH))
                            .h_full()
                            .bg(theme::separator()),
                    ),
            )
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
