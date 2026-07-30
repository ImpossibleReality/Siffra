//! Selectable, syntax-highlighted calculator examples.

use crate::highlight::{Token, highlight_line};
use crate::theme;
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, Entity, EventEmitter,
    FocusHandle, Focusable, FontWeight, GlobalElementId, HighlightStyle, InspectorElementId,
    InteractiveElement, IntoElement, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, ShapedLine, SharedString,
    Stateful, StatefulInteractiveElement, Style, Styled, StyledText, TextRun, Window, div, px,
    relative, rgb, rgba,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

pub struct CodeLineActivated;

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

pub struct SelectableCodeLine {
    focus_handle: FocusHandle,
    content: SharedString,
    mono_family: SharedString,
    selection: Range<usize>,
    selection_reversed: bool,
    drag: Option<SelectionDrag>,
    last_layout: Option<ShapedLine>,
    last_layout_selection: Range<usize>,
    last_bounds: Option<Bounds<Pixels>>,
}

impl EventEmitter<CodeLineActivated> for SelectableCodeLine {}

impl SelectableCodeLine {
    pub fn new(
        content: impl Into<SharedString>,
        mono_family: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: content.into(),
            mono_family: mono_family.into(),
            selection: 0..0,
            selection_reversed: false,
            drag: None,
            last_layout: None,
            last_layout_selection: 0..0,
            last_bounds: None,
        }
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        let changed = !self.selection.is_empty() || self.drag.is_some();
        self.selection = 0..0;
        self.selection_reversed = false;
        self.drag = None;
        if changed {
            cx.notify();
        }
    }

    fn index_for_position(&self, position: gpui::Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (&self.last_bounds, &self.last_layout) else {
            return 0;
        };
        if position.x <= bounds.left() {
            return 0;
        }
        if position.x >= bounds.right() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn word_range_at(&self, offset: usize) -> Range<usize> {
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

    fn drag_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_ref() else {
            return;
        };
        let reach = match drag.granularity {
            SelectionGranularity::Character => offset..offset,
            SelectionGranularity::Word => self.word_range_at(offset),
            SelectionGranularity::Line => 0..self.content.len(),
        };
        let anchor = drag.anchor.clone();
        self.selection = anchor.start.min(reach.start)..anchor.end.max(reach.end);
        self.selection_reversed = reach.start < anchor.start;
        cx.notify();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        cx.emit(CodeLineActivated);
        let offset = self.index_for_position(event.position);
        let (granularity, anchor) = match event.click_count {
            0 | 1 if event.modifiers.shift => {
                let fixed = if self.selection_reversed {
                    self.selection.end
                } else {
                    self.selection.start
                };
                (SelectionGranularity::Character, fixed..fixed)
            }
            0 | 1 => (SelectionGranularity::Character, offset..offset),
            2 => (SelectionGranularity::Word, self.word_range_at(offset)),
            _ => (SelectionGranularity::Line, 0..self.content.len()),
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

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !event.keystroke.modifiers.secondary() {
            return;
        }
        if event.keystroke.key.eq_ignore_ascii_case("a") {
            self.selection = 0..self.content.len();
            self.selection_reversed = false;
            cx.stop_propagation();
            cx.notify();
        } else if event.keystroke.key.eq_ignore_ascii_case("c") && !self.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selection.clone()].to_string(),
            ));
            cx.stop_propagation();
        }
    }
}

impl Focusable for SelectableCodeLine {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SelectableCodeLine {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h(px(1.445 * theme::REM))
            .font_family(self.mono_family.clone())
            .text_size(px(0.85 * theme::REM))
            .line_height(px(1.445 * theme::REM))
            .text_color(rgb(0xcdd4d7))
            .cursor(CursorStyle::IBeam)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .child(SelectableCodeLineElement { input: cx.entity() })
    }
}

struct SelectableCodeLineElement {
    input: Entity<SelectableCodeLine>,
}

impl IntoElement for SelectableCodeLineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableCodeLineElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<ShapedLine>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selection = input.selection.clone();
        if input.last_layout_selection == selection
            && let Some(line) = input.last_layout.as_ref()
        {
            return Some(line.clone());
        }
        let spans = highlight_line(&content);
        let style = window.text_style();
        let mut boundaries = vec![0, content.len()];
        for span in &spans {
            boundaries.push(span.range.start);
            boundaries.push(span.range.end);
        }
        if !selection.is_empty() {
            boundaries.push(selection.start);
            boundaries.push(selection.end);
        }
        boundaries.sort_unstable();
        boundaries.dedup();

        let runs = boundaries
            .windows(2)
            .filter_map(|boundary| {
                let start = boundary[0];
                let end = boundary[1];
                (start < end).then(|| {
                    let token = spans
                        .iter()
                        .find(|span| span.range.start <= start && start < span.range.end)
                        .map(|span| span.token);
                    let mut font = style.font();
                    if token == Some(Token::Header) {
                        font.weight = FontWeight::BOLD;
                    }
                    TextRun {
                        len: end - start,
                        font,
                        color: token.map_or(style.color, token_color),
                        background_color: (selection.start <= start
                            && end <= selection.end
                            && !selection.is_empty())
                        .then_some(theme::selection().into()),
                        underline: None,
                        strikethrough: None,
                    }
                })
            })
            .collect::<Vec<_>>();
        let font_size = style.font_size.to_pixels(window.rem_size());
        Some(
            window
                .text_system()
                .shape_line(content, font_size, &runs, None),
        )
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        line: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let line = line
            .take()
            .expect("selectable code line is shaped in prepaint");
        line.paint_background(bounds.origin, window.line_height(), window, cx)
            .ok();
        line.paint(bounds.origin, window.line_height(), window, cx)
            .ok();
        self.input.update(cx, |input, _| {
            input.last_layout = Some(line);
            input.last_layout_selection = input.selection.clone();
            input.last_bounds = Some(bounds);
        });
    }
}

/// A calculation line in the colours the editor gives it, for text that is only
/// there to be read. Nothing about it is selectable, which is why it is drawn as
/// one styled run rather than shaped by hand like [`SelectableCodeLine`].
pub fn highlighted_line(content: SharedString) -> StyledText {
    let highlights: Vec<_> = highlight_line(&content)
        .into_iter()
        .map(|span| {
            (
                span.range,
                HighlightStyle {
                    color: Some(token_color(span.token)),
                    font_weight: (span.token == Token::Header).then_some(FontWeight::BOLD),
                    ..Default::default()
                },
            )
        })
        .collect();
    StyledText::new(content).with_highlights(highlights)
}

pub struct CodeBlockLine {
    pub input: Entity<SelectableCodeLine>,
    pub result: Option<SharedString>,
}

impl CodeBlockLine {
    pub fn new(input: Entity<SelectableCodeLine>) -> Self {
        Self {
            input,
            result: None,
        }
    }

    pub fn with_result(input: Entity<SelectableCodeLine>, result: impl Into<SharedString>) -> Self {
        Self {
            input,
            result: Some(result.into()),
        }
    }
}

pub fn code_block(
    id: impl Into<ElementId>,
    lines: impl IntoIterator<Item = CodeBlockLine>,
) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .px(px(0.7 * theme::REM))
        .py(px(0.5 * theme::REM))
        .overflow_x_scroll()
        .bg(rgba(0x0000002e))
        .border_1()
        .border_color(rgb(0x33363f))
        .rounded(px(0.4 * theme::REM))
        .text_size(px(0.85 * theme::REM))
        .line_height(px(1.445 * theme::REM))
        .children(lines.into_iter().map(|line| {
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(theme::REM))
                .whitespace_nowrap()
                .child(div().flex_1().text_color(rgb(0xcdd4d7)).child(line.input))
                .children(line.result.map(|result| {
                    div()
                        .flex_none()
                        .text_right()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0x98c379))
                        .child(result)
                }))
        }))
}

fn token_color(token: Token) -> gpui::Hsla {
    match token {
        Token::Header => theme::header().into(),
        Token::Comment => theme::comment().into(),
        Token::Chemical => theme::chemical().into(),
        Token::Number => theme::number().into(),
        Token::Operation => theme::operation().into(),
    }
}
