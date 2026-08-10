use super::input_element::InputElement;
use super::layout::{Layout, Metrics};
use super::{Editor, result};
use crate::calculation::LineOutput;
use crate::ui::popover::{below_right, popover_button};
use crate::ui::theme;
use gpui::{
    ClickEvent, Context, CursorStyle, Div, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use std::ops::Range;
use std::rc::Rc;

impl Editor {
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
        let row_top = px(theme::GUTTER) + layout.line_top(index) + self.scroll.offset().y;
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
