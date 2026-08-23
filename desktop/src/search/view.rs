use super::matching::{Haystack, MatchLine, Row};
use crate::documents::Document;
use crate::ui::assets::icon;
use crate::ui::checkbox::checkbox;
use crate::ui::code_block::highlighted_line;
use crate::ui::modal::{ModalSize, fitted_panel, overlay};
use crate::ui::single_line_input::{FieldMetrics, FieldState, SingleLineInput, ValueChanged};
use crate::ui::theme;
use gpui::{
    AnyElement, App, AppContext, Context, CursorStyle, Div, Entity, EventEmitter, FocusHandle,
    Focusable, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ListAlignment,
    ListSizingBehavior, ListState, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement,
    Render, SharedString, Stateful, StatefulInteractiveElement, Styled, Subscription, Window, div,
    list, prelude::FluentBuilder, px, rgb, rgba, svg,
};

const PANEL_WIDTH: f32 = 560.0;
const PANEL_HEIGHT: f32 = 28.0 * theme::REM;
const OVERLAY_TOP_MIN: f32 = 3.0 * theme::REM;
const OVERLAY_TOP_FRACTION: f32 = 0.12;
const OVERLAY_SIDE: f32 = theme::REM;
const LIST_OVERDRAW: f32 = PANEL_HEIGHT;

pub enum SearchEvent {
    Close,
    Open(u64),
    Delete(u64),
}

pub struct SearchModal {
    query: Entity<SingleLineInput>,
    mono_family: SharedString,
    haystack: Haystack,
    needle: String,
    rows: Vec<Row>,
    /// Whether the listing keeps the unnamed calculations older than a day.
    show_unnamed: bool,
    selected: usize,
    list: ListState,
    _subscription: Subscription,
}

impl EventEmitter<SearchEvent> for SearchModal {}

impl SearchModal {
    pub fn new(mono_family: SharedString, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| {
            SingleLineInput::new(
                "search-query",
                "Search calculations…",
                query_field_metrics(),
                query_field_chrome,
                cx,
            )
        });
        let subscription = cx.subscribe(&query, |modal, _, _: &ValueChanged, cx| {
            modal.selected = 0;
            modal.rebuild_rows(cx);
        });

        Self {
            query,
            mono_family,
            haystack: Haystack::new(Vec::new()),
            needle: String::new(),
            rows: Vec::new(),
            show_unnamed: false,
            selected: 0,
            list: ListState::new(0, ListAlignment::Top, px(LIST_OVERDRAW)),
            _subscription: subscription,
        }
    }

    pub fn open(&mut self, documents: Vec<Document>, cx: &mut Context<Self>) {
        self.selected = 0;
        self.query.update(cx, |query, cx| {
            query.set_value("", cx);
            query.request_focus(cx);
        });
        self.set_documents(documents, cx);
    }

    pub fn set_documents(&mut self, documents: Vec<Document>, cx: &mut Context<Self>) {
        self.haystack = Haystack::new(documents);
        self.rebuild_rows(cx);
    }

    fn rebuild_rows(&mut self, cx: &mut Context<Self>) {
        self.needle = self.query.read(cx).value().trim().to_string();
        self.rows = self.haystack.rows(&self.needle, self.show_unnamed);
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        self.list.reset(self.rows.len());
        self.list.scroll_to_reveal_item(self.selected);
        cx.notify();
    }

    fn toggle_unnamed(&mut self, cx: &mut Context<Self>) {
        self.show_unnamed = !self.show_unnamed;
        self.selected = 0;
        self.rebuild_rows(cx);
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let count = self.rows.len() as isize;
        let selected = (self.selected as isize + delta).rem_euclid(count);
        self.selected = selected as usize;
        self.list.scroll_to_reveal_item(self.selected);
        cx.notify();
    }

    fn choose(&mut self, cx: &mut Context<Self>) {
        if let Some(row) = self.rows.get(self.selected) {
            cx.emit(SearchEvent::Open(row.id));
        }
    }

    fn delete(&mut self, id: u64, cx: &mut Context<Self>) {
        cx.emit(SearchEvent::Delete(id));
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => {
                cx.stop_propagation();
                cx.emit(SearchEvent::Close);
            }
            "down" => {
                cx.stop_propagation();
                self.move_selection(1, cx);
            }
            "up" => {
                cx.stop_propagation();
                self.move_selection(-1, cx);
            }
            "enter" => {
                cx.stop_propagation();
                self.choose(cx);
            }
            "backspace" if event.keystroke.modifiers.secondary() => {
                if let Some(row) = self.rows.get(self.selected) {
                    cx.stop_propagation();
                    let id = row.id;
                    self.delete(id, cx);
                }
            }
            _ => {}
        }
    }

    fn render_row(&self, index: usize, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(row) = self.rows.get(index) else {
            return div().into_any_element();
        };
        let active = self.selected == index;
        let id = row.id;
        let matches = if row.matched_content {
            self.haystack.match_lines(row.entry, &self.needle)
        } else {
            Vec::new()
        };

        div()
            .id(("search-row", index))
            .px(px(0.55 * theme::REM))
            .py(px(0.4 * theme::REM))
            .rounded(px(0.4 * theme::REM))
            .cursor(CursorStyle::Arrow)
            .when(active, |row| row.bg(rgb(0x383d46)))
            .on_mouse_move(cx.listener(move |modal, _: &MouseMoveEvent, _, cx| {
                if modal.selected != index {
                    modal.selected = index;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(SearchEvent::Open(id));
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(0.6 * theme::REM))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(0.86 * theme::REM))
                            .text_color(if active { rgb(0xe6ebee) } else { rgb(0xcdd4d7) })
                            .child(row.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(0.7 * theme::REM))
                            .text_color(rgb(0x6b7280))
                            .child(row.edited.clone()),
                    )
                    .child(self.render_delete(index, id, active, cx)),
            )
            .children((!matches.is_empty()).then(|| self.render_matches(index, &matches)))
            .into_any_element()
    }

    fn render_delete(
        &self,
        index: usize,
        id: u64,
        active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        const GROUP: &str = "search-row-delete";

        div()
            .id(("search-delete", index))
            .group(GROUP)
            .flex_none()
            .size(px(1.25 * theme::REM))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(0.3 * theme::REM))
            .cursor(CursorStyle::Arrow)
            .hover(|style| style.bg(rgba(0xe06c7524)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |modal, _, _, cx| {
                modal.delete(id, cx);
            }))
            .child(
                svg()
                    .path(icon::CLOSE)
                    .size(px(0.8 * theme::REM))
                    .flex_none()
                    .text_color(if active {
                        rgb(0x6b7280).into()
                    } else {
                        gpui::transparent_black()
                    })
                    .group_hover(GROUP, |style| style.text_color(rgb(0xe06c75))),
            )
    }

    fn render_matches(&self, index: usize, lines: &[MatchLine]) -> impl IntoElement {
        div()
            .id(("search-matches", index))
            .mt(px(0.35 * theme::REM))
            .px(px(0.5 * theme::REM))
            .py(px(0.3 * theme::REM))
            .bg(rgb(0x1b1d22))
            .border_1()
            .border_color(rgb(0x2f333c))
            .rounded(px(0.35 * theme::REM))
            .font_family(self.mono_family.clone())
            .text_size(px(0.72 * theme::REM))
            .line_height(px(1.19 * theme::REM))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xcdd4d7))
            .children(
                lines
                    .iter()
                    .flat_map(|line| {
                        [
                            line.skipped_before.then(gap_marker),
                            Some(
                                div()
                                    .w_full()
                                    .truncate()
                                    .child(highlighted_line(line.text.clone()))
                                    .into_any_element(),
                            ),
                        ]
                    })
                    .flatten(),
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(0.85 * theme::REM))
            .px(px(0.85 * theme::REM))
            .py(px(0.4 * theme::REM))
            .border_t_1()
            .border_color(rgb(0x2b2e36))
            .text_size(px(0.68 * theme::REM))
            .text_color(rgb(0x6b7280))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(0.85 * theme::REM))
                    .child(hint("↑↓", " navigate"))
                    .child(hint("↵", " open"))
                    .child(hint(format!("{MOD_KEY}⌫"), " delete"))
                    .child(hint("esc", " close")),
            )
            .child(
                checkbox("search-show-unnamed", "Show Unnamed", self.show_unnamed)
                    .flex_none()
                    .text_size(px(0.68 * theme::REM))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                        }),
                    )
                    .on_click(cx.listener(|modal, _, _, cx| {
                        modal.toggle_unnamed(cx);
                    })),
            )
    }

    fn render_empty(&self) -> impl IntoElement {
        div()
            .px(px(0.6 * theme::REM))
            .py(px(theme::REM))
            .text_center()
            .text_size(px(0.82 * theme::REM))
            .text_color(rgb(0x6b7280))
            .child(if self.haystack.is_empty() {
                SharedString::new_static("No saved calculations yet.")
            } else if self.needle.is_empty() {
                SharedString::new_static("Only unnamed calculations older than a day are saved.")
            } else {
                SharedString::from(format!("No calculations match “{}”.", self.needle))
            })
    }
}

impl Render for SearchModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size().height;
        let top = px(OVERLAY_TOP_MIN.max(f32::from(viewport) * OVERLAY_TOP_FRACTION));
        let height = (viewport - top - px(OVERLAY_SIDE)).max(px(0.0));

        let empty = self.rows.is_empty().then(|| self.render_empty());
        let rows = list(
            self.list.clone(),
            cx.processor(|modal, index, window, cx| modal.render_row(index, window, cx)),
        )
        .with_sizing_behavior(ListSizingBehavior::Infer);

        overlay("search-overlay")
            .items_start()
            .pt(top)
            .key_context("SiffraSearch")
            .capture_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    cx.emit(SearchEvent::Close);
                }),
            )
            .child(
                fitted_panel(
                    "search-panel",
                    ModalSize::new(px(PANEL_WIDTH), px(PANEL_HEIGHT.min(f32::from(height)))),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(0.5 * theme::REM))
                        .px(px(0.85 * theme::REM))
                        .py(px(0.7 * theme::REM))
                        .border_b_1()
                        .border_color(rgb(0x2b2e36))
                        .child(
                            svg()
                                .path(icon::SEARCH)
                                .size(px(theme::ICON_SIZE))
                                .flex_none()
                                .text_color(rgb(0x6b7280)),
                        )
                        .child(self.query.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .px(px(0.35 * theme::REM))
                        .child(match empty {
                            Some(empty) => div()
                                .py(px(0.35 * theme::REM))
                                .child(empty)
                                .into_any_element(),
                            None => rows
                                .flex_1()
                                .min_h_0()
                                .py(px(0.35 * theme::REM))
                                .into_any_element(),
                        }),
                )
                .child(self.render_footer(cx)),
            )
    }
}

impl Focusable for SearchModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.query.read(cx).focus_handle()
    }
}

#[cfg(target_os = "macos")]
const MOD_KEY: &str = "⌘";
#[cfg(not(target_os = "macos"))]
const MOD_KEY: &str = "Ctrl ";

fn hint(keys: impl Into<SharedString>, label: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .whitespace_nowrap()
        .child(div().text_color(rgb(0x8a9196)).child(keys.into()))
        .child(label)
}

fn gap_marker() -> gpui::AnyElement {
    div()
        .relative()
        .h(px(0.85 * theme::REM))
        .my(px(0.1 * theme::REM))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .absolute()
                .top(px(0.425 * theme::REM))
                .left_0()
                .right_0()
                .h(px(1.0))
                .bg(rgb(0x2f333c)),
        )
        .child(
            div()
                .relative()
                .px(px(0.3 * theme::REM))
                .flex()
                .items_center()
                .justify_center()
                .h_full()
                .bg(rgb(0x1b1d22))
                .border_1()
                .border_color(rgb(0x2f333c))
                .rounded(px(0.25 * theme::REM))
                .text_size(px(0.6 * theme::REM))
                .line_height(px(0.6 * theme::REM))
                .text_color(rgb(0x6b7280))
                .child("…"),
        )
        .into_any_element()
}

fn query_field_metrics() -> FieldMetrics {
    FieldMetrics {
        padding_x: px(0.0),
        padding_y: px(0.0),
        caret_height: px(1.29 * theme::REM),
        selection: theme::title_selection(),
        selection_text: Some(theme::title_selection_text()),
        placeholder: rgb(0x6b7280),
        centered: false,
    }
}

fn query_field_chrome(field: Stateful<Div>, _: FieldState) -> Stateful<Div> {
    field
        .flex_1()
        .min_w_0()
        .overflow_x_scroll()
        .text_size(px(0.95 * theme::REM))
        .line_height(px(1.29 * theme::REM))
        .text_color(rgb(0xcdd4d7))
        .cursor(CursorStyle::IBeam)
}
