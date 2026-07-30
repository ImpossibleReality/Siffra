//! The palette that finds a saved calculation.
//!
//! It matches a query against every name and, failing that, against the lines of
//! every calculation, so a file can be found by something it works out as well as
//! by what it is called. The lines that matched are shown under the name, in the
//! colours the editor gives them.
//!
//! The store is the workspace's; this modal is handed a list and reports what the
//! user chose to do with it.

use crate::assets::icon;
use crate::documents::{Document, now_millis};
use crate::fuzzy;
use crate::relative_time::relative_time;
use crate::theme;
use crate::ui::code_block::highlighted_line;
use crate::ui::modal::{ModalSize, fitted_panel, overlay};
use crate::ui::single_line_input::{FieldMetrics, FieldState, SingleLineInput, ValueChanged};
use gpui::{
    App, AppContext, Context, CursorStyle, Div, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement, Render, ScrollHandle, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::FluentBuilder, px, rgb,
    rgba, svg,
};
use std::cmp::Reverse;

/// How many matching lines to show under a calculation.
const MAX_MATCH_LINES: usize = 4;

const PANEL_WIDTH: f32 = 560.0;
const PANEL_HEIGHT: f32 = 28.0 * theme::REM;
/// The palette starts below whichever is larger: the minimum top inset or 12% of
/// the viewport height.
const OVERLAY_TOP_MIN: f32 = 3.0 * theme::REM;
const OVERLAY_TOP_FRACTION: f32 = 0.12;
const OVERLAY_SIDE: f32 = theme::REM;

/// What the modal asks the workspace to do. Everything that touches the store
/// happens there, so a delete comes back as a fresh list.
pub enum SearchEvent {
    Close,
    Open(u64),
    Delete(u64),
}

struct MatchLine {
    text: SharedString,
    /// Set when calculation lines were skipped before this one. Blank lines are
    /// not a skip: there is nothing hidden to stand in for.
    skipped_before: bool,
}

struct Row {
    id: u64,
    title: SharedString,
    edited: SharedString,
    /// The lines that matched the query. Empty when the name itself matched.
    lines: Vec<MatchLine>,
}

pub struct SearchModal {
    query: Entity<SingleLineInput>,
    mono_family: SharedString,
    /// Every saved calculation, as the workspace last read them.
    all: Vec<Document>,
    rows: Vec<Row>,
    selected: usize,
    list: ScrollHandle,
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
            // Query changes can reorder the rows, so restart at the top.
            modal.selected = 0;
            modal.rebuild_rows(cx);
        });

        Self {
            query,
            mono_family,
            all: Vec::new(),
            rows: Vec::new(),
            selected: 0,
            list: ScrollHandle::new(),
            _subscription: subscription,
        }
    }

    /// Shows the modal from the top: an empty query, the newest calculation
    /// highlighted, and the caret in the field.
    pub fn open(&mut self, documents: Vec<Document>, cx: &mut Context<Self>) {
        self.selected = 0;
        self.query.update(cx, |query, cx| {
            query.set_value("", cx);
            query.request_focus(cx);
        });
        self.set_documents(documents, cx);
    }

    /// Replaces the list the modal is matching against, keeping the highlight on
    /// whatever row has taken the place of the one it was on.
    pub fn set_documents(&mut self, documents: Vec<Document>, cx: &mut Context<Self>) {
        self.all = documents;
        self.rebuild_rows(cx);
    }

    fn rebuild_rows(&mut self, cx: &mut Context<Self>) {
        let query = self.query.read(cx).value().trim().to_string();
        self.rows = rows_for(&query, &self.all);
        // Filtering can remove the selected row.
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let count = self.rows.len() as isize;
        let selected = (self.selected as isize + delta).rem_euclid(count);
        self.selected = selected as usize;
        self.list.scroll_to_item(self.selected);
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
            // ⌘⌫ deletes the highlighted calculation rather than a character of
            // the query, which is why it is taken before the field sees it.
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

    fn render_row(&self, index: usize, row: &Row, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.selected == index;
        let id = row.id;

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
            .children((!row.lines.is_empty()).then(|| self.render_matches(index, &row.lines)))
    }

    /// Shows delete only for the highlighted row.
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
            // The press is taken here so it never reads as a press on the row,
            // which would open the calculation being deleted.
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
                    // A transparent glyph hides the control without changing the
                    // row's layout.
                    .text_color(if active {
                        rgb(0x6b7280).into()
                    } else {
                        gpui::transparent_black()
                    })
                    .group_hover(GROUP, |style| style.text_color(rgb(0xe06c75))),
            )
    }

    /// Renders matched lines as highlighted code, marking omitted lines between
    /// non-adjacent matches.
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

    fn render_hints(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap(px(0.85 * theme::REM))
            .px(px(0.85 * theme::REM))
            .py(px(0.4 * theme::REM))
            .border_t_1()
            .border_color(rgb(0x2b2e36))
            .text_size(px(0.68 * theme::REM))
            .text_color(rgb(0x6b7280))
            .child(hint("↑↓", " navigate"))
            .child(hint("↵", " open"))
            .child(hint(format!("{MOD_KEY}⌫"), " delete"))
            .child(hint("esc", " close"))
    }

    fn render_empty(&self, cx: &App) -> impl IntoElement {
        let query = self.query.read(cx).value().trim().to_string();
        div()
            .px(px(0.6 * theme::REM))
            .py(px(theme::REM))
            .text_center()
            .text_size(px(0.82 * theme::REM))
            .text_color(rgb(0x6b7280))
            .child(if self.all.is_empty() {
                SharedString::new_static("No saved calculations yet.")
            } else {
                SharedString::from(format!("No calculations match “{query}”."))
            })
    }
}

impl Render for SearchModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size().height;
        let top = px(OVERLAY_TOP_MIN.max(f32::from(viewport) * OVERLAY_TOP_FRACTION));
        let height = (viewport - top - px(OVERLAY_SIDE)).max(px(0.0));

        let rows: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| self.render_row(index, row, cx).into_any_element())
            .collect();
        let empty = rows.is_empty().then(|| self.render_empty(cx));

        overlay("search-overlay")
            .items_start()
            .pt(top)
            // Named so the query field's own ⌘⌫ can be bound out from under it:
            // in here that keystroke deletes the highlighted calculation, and a
            // binding resolves before any of these keys are looked at.
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
                        .id("search-list")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.list)
                        .p(px(0.35 * theme::REM))
                        .children(rows)
                        .children(empty),
                )
                .child(self.render_hints()),
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

/// Marks non-empty lines omitted between matching lines.
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

/// The list the modal shows, in the order it shows it.
///
/// A name match stands on its own — no lines are shown under it, and it outranks
/// anything found in the body of a calculation.
fn rows_for(query: &str, all: &[Document]) -> Vec<Row> {
    let now = now_millis();

    if query.is_empty() {
        return all
            .iter()
            .map(|document| row_at(document, Vec::new(), now))
            .collect();
    }

    let mut by_name: Vec<(Row, i32)> = Vec::new();
    let mut by_content: Vec<(Row, i32)> = Vec::new();

    for document in all {
        if let Some(score) = fuzzy::score(query, &document.title) {
            by_name.push((row_at(document, Vec::new(), now), score));
            continue;
        }

        let lines: Vec<&str> = document.content.split('\n').collect();
        let mut hits: Vec<(usize, &str, i32)> = lines
            .iter()
            .enumerate()
            .map(|(index, line)| (index, line.trim()))
            .filter(|(_, line)| !line.is_empty())
            .filter_map(|(index, line)| fuzzy::score(query, line).map(|score| (index, line, score)))
            .collect();
        // Rank by the best hits, then restore source order for display.
        hits.sort_by_key(|(_, _, score)| Reverse(*score));
        hits.truncate(MAX_MATCH_LINES);
        if hits.is_empty() {
            continue;
        }

        let best = hits[0].2;
        hits.sort_by_key(|(index, _, _)| *index);
        let matches = hits
            .iter()
            .enumerate()
            .map(|(position, (index, text, _))| MatchLine {
                text: SharedString::from((*text).to_string()),
                skipped_before: position > 0
                    && lines[hits[position - 1].0 + 1..*index]
                        .iter()
                        .any(|line| !line.trim().is_empty()),
            })
            .collect();
        by_content.push((row_at(document, matches, now), best));
    }

    by_name.sort_by_key(|(_, score)| Reverse(*score));
    by_content.sort_by_key(|(_, score)| Reverse(*score));
    by_name
        .into_iter()
        .chain(by_content)
        .map(|(row, _)| row)
        .collect()
}

fn row_at(document: &Document, lines: Vec<MatchLine>, now: i64) -> Row {
    Row {
        id: document.id,
        title: SharedString::from(document.title.clone()),
        edited: SharedString::from(relative_time(document.updated_at, now)),
        lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(id: u64, title: &str, content: &str) -> Document {
        Document {
            id,
            title: title.to_string(),
            content: content.to_string(),
            cursor: 0,
            created_at: 0,
            updated_at: id as i64,
        }
    }

    #[test]
    fn an_empty_query_lists_everything_in_the_order_it_was_given() {
        let all = vec![document(2, "Two", ""), document(1, "One", "")];
        let rows = rows_for("", &all);
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [2, 1],
            "the store hands the list over newest first"
        );
        assert!(rows.iter().all(|row| row.lines.is_empty()));
    }

    #[test]
    fn a_name_match_outranks_anything_found_inside_a_calculation() {
        let all = vec![
            document(1, "Groceries", "12 kg to lb"),
            document(2, "Trip budget", "groceries = 40 USD"),
        ];
        let rows = rows_for("groceries", &all);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, 1);
        assert!(rows[0].lines.is_empty(), "a name match shows no lines");
        assert_eq!(rows[1].lines.len(), 1);
        assert_eq!(rows[1].lines[0].text, "groceries = 40 USD");
    }

    #[test]
    fn only_the_best_few_lines_are_shown_and_they_read_in_order() {
        let content = (0..10)
            .map(|index| format!("kg {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let all = vec![document(1, "Weights", &content)];
        let rows = rows_for("kg", &all);
        let lines = &rows[0].lines;
        assert_eq!(lines.len(), MAX_MATCH_LINES);
        let shown: Vec<_> = lines.iter().map(|line| line.text.to_string()).collect();
        let mut sorted = shown.clone();
        sorted.sort();
        assert_eq!(shown, sorted, "lines keep the order they were written in");
    }

    #[test]
    fn a_skipped_line_is_marked_but_a_blank_one_is_not() {
        let all = vec![
            document(1, "Gaps", "kg 1\n3 + 3\nkg 2"),
            document(2, "Blanks", "kg 1\n\nkg 2"),
        ];
        let gapped = rows_for("kg", &all);
        let with_gap = gapped.iter().find(|row| row.id == 1).expect("row");
        assert!(!with_gap.lines[0].skipped_before);
        assert!(
            with_gap.lines[1].skipped_before,
            "a calculation line was left out between the two matches"
        );

        let with_blank = gapped.iter().find(|row| row.id == 2).expect("row");
        assert!(
            !with_blank.lines[1].skipped_before,
            "a blank line hides nothing, so there is nothing to stand in for"
        );
    }

    #[test]
    fn calculations_that_match_nowhere_are_left_out() {
        let all = vec![document(1, "Trip", "2 + 2")];
        assert!(rows_for("zzz", &all).is_empty());
    }
}
