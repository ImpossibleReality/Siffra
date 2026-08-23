//! The searchable help dialog view.
//!
//! Catalog entries come from `siffra::catalog`; the introductory overview is
//! authored in the [`super::content`] module.

use super::content::*;
use super::rows::{
    HelpRow, Tab, changed_rows, default_open_sections, non_search_rows, overview_entry,
    search_rows, section_key,
};
use super::search_index::{
    SearchIndex, build_search_index, scored_document_indices, searchable_units,
};
use crate::text::{Left, Right};
use crate::ui::assets::icon;
use crate::ui::code_block::{CodeBlockLine, CodeLineActivated, SelectableCodeLine, code_block};
use crate::ui::modal::{ModalSize, body as modal_body, close_button, header, overlay, panel};
use crate::ui::single_line_input::{FieldMetrics, FieldState, SingleLineInput, ValueChanged};
use crate::ui::theme;
use crate::ui::tooltip::TextTooltip;
use crate::updates::VersionButton;
use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, Context, CursorStyle, Div, Entity,
    EntityId, EventEmitter, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ListAlignment, ListState, MouseButton, MouseDownEvent, ParentElement, Pixels,
    Render, SharedString, Stateful, StatefulInteractiveElement, Styled, Subscription, Task,
    Transformation, Window, div, list, prelude::FluentBuilder, px, radians, rgb, rgba, svg,
};
use siffra::catalog::{
    Catalog, CatalogConstant, CatalogExample, CatalogFunction, CatalogUnit, catalog,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct CloseHelp;

struct ChevronAnimation {
    key: String,
    opening: bool,
    serial: u64,
    started: Instant,
}

pub struct HelpModal {
    tab: Tab,
    search: Entity<SingleLineInput>,
    _search_subscription: Subscription,
    catalog: Catalog,
    mono_family: SharedString,
    example_inputs: HashMap<String, Entity<SelectableCodeLine>>,
    active_example: Option<EntityId>,
    _example_subscriptions: Vec<Subscription>,
    open_sections: HashSet<String>,
    searchable_units: Vec<CatalogUnit>,
    search_index: Arc<SearchIndex>,
    rows: Vec<HelpRow>,
    list_state: ListState,
    search_generation: u64,
    search_task: Option<Task<()>>,
    chevron_animation: Option<ChevronAnimation>,
    chevron_serial: u64,
    version: Entity<VersionButton>,
}

impl EventEmitter<CloseHelp> for HelpModal {}

impl HelpModal {
    pub fn new(
        mono_family: SharedString,
        version: Entity<VersionButton>,
        cx: &mut Context<Self>,
    ) -> Self {
        let catalog = catalog();
        let open_sections = default_open_sections(&catalog);
        let searchable_units = searchable_units(&catalog);
        let search_index = Arc::new(build_search_index(&catalog, &searchable_units));
        let search = cx.new(|cx| {
            SingleLineInput::new(
                "help-search",
                "Search overview…",
                search_field_metrics(),
                search_field_chrome,
                cx,
            )
        });
        let search_subscription = cx.subscribe(&search, |modal, search, _: &ValueChanged, cx| {
            let query = search.read(cx).value().to_string();
            modal.start_search(query, cx);
        });
        let example_inputs = build_example_inputs(&catalog, mono_family.clone(), cx);
        let example_subscriptions = example_inputs
            .values()
            .map(|input| {
                cx.subscribe(input, |modal, input, _: &CodeLineActivated, cx| {
                    modal.activate_example(input.entity_id(), cx);
                })
            })
            .collect();
        let rows = non_search_rows(Tab::Overview, &catalog, &open_sections);
        let list_state = ListState::new(rows.len(), ListAlignment::Top, px(12.0 * theme::REM));
        Self {
            tab: Tab::Overview,
            search,
            _search_subscription: search_subscription,
            open_sections,
            catalog,
            mono_family,
            example_inputs,
            active_example: None,
            _example_subscriptions: example_subscriptions,
            searchable_units,
            search_index,
            rows,
            list_state,
            search_generation: 0,
            search_task: None,
            chevron_animation: None,
            chevron_serial: 0,
            version,
        }
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
        self.version.update(cx, VersionButton::close_menu);
        self.tab = Tab::Overview;
        self.search.update(cx, |search, cx| {
            search.set_placeholder("Search overview…", cx);
            search.set_value("", cx);
            search.request_focus(cx);
        });
        self.open_sections = default_open_sections(&self.catalog);
        self.clear_example_selections(cx);
        self.cancel_search();
        self.chevron_animation = None;
        self.rebuild_non_search_rows();
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        cx.emit(CloseHelp);
    }

    fn handle_overlay_key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            cx.stop_propagation();
            self.close(cx);
        }
    }

    fn step_tab(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let last = Tab::ALL.len() as isize - 1;
        let index = (self.tab.index() as isize + delta).clamp(0, last) as usize;
        if Tab::ALL[index] != self.tab {
            self.set_tab(Tab::ALL[index], window, cx);
        }
    }

    fn set_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        self.search.update(cx, |search, cx| {
            search.set_placeholder(format!("Search {}…", tab.key()), cx);
            search.set_value("", cx);
        });
        self.clear_example_selections(cx);
        self.cancel_search();
        self.chevron_animation = None;
        self.rebuild_non_search_rows();
        self.search.read(cx).focus_handle().focus(window);
        cx.notify();
    }

    fn clear_example_selections(&mut self, cx: &mut Context<Self>) {
        for input in self.example_inputs.values() {
            input.update(cx, SelectableCodeLine::clear_selection);
        }
        self.active_example = None;
    }

    fn activate_example(&mut self, active: EntityId, cx: &mut Context<Self>) {
        if self.active_example == Some(active) {
            return;
        }
        if let Some(previous) = self.active_example
            && let Some(input) = self
                .example_inputs
                .values()
                .find(|input| input.entity_id() == previous)
        {
            input.update(cx, SelectableCodeLine::clear_selection);
        }
        self.active_example = Some(active);
    }

    fn is_open(&self, tab: Tab, category: &str) -> bool {
        self.open_sections.contains(&section_key(tab, category))
    }

    fn toggle_section(&mut self, key: String, cx: &mut Context<Self>) {
        let animation_key = key.clone();
        let opening = if self.open_sections.remove(&key) {
            false
        } else {
            self.open_sections.insert(key);
            true
        };
        self.chevron_serial = self.chevron_serial.wrapping_add(1);
        self.chevron_animation = Some(ChevronAnimation {
            key: animation_key,
            opening,
            serial: self.chevron_serial,
            started: Instant::now(),
        });
        self.replace_rows_preserving_scroll(non_search_rows(
            self.tab,
            &self.catalog,
            &self.open_sections,
        ));
        cx.notify();
    }

    fn cancel_search(&mut self) {
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_task = None;
    }

    fn replace_rows(&mut self, rows: Vec<HelpRow>) {
        self.rows = rows;
        self.list_state.reset(self.rows.len());
    }

    fn replace_rows_preserving_scroll(&mut self, rows: Vec<HelpRow>) {
        let (old_range, new_count) = changed_rows(&self.rows, &rows);
        self.list_state.splice(old_range, new_count);
        self.rows = rows;
    }

    fn rebuild_non_search_rows(&mut self) {
        self.replace_rows(non_search_rows(
            self.tab,
            &self.catalog,
            &self.open_sections,
        ));
    }

    fn start_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.cancel_search();
        let query = query.trim().to_string();
        if query.is_empty() {
            self.rebuild_non_search_rows();
            return;
        }

        let generation = self.search_generation;
        let tab = self.tab;
        let index = self.search_index.clone();
        let work_query = query.clone();
        let work = cx.background_spawn(async move {
            scored_document_indices(&work_query, index.documents(tab))
        });
        self.search_task = Some(cx.spawn(async move |modal, cx| {
            let indices = work.await;
            modal
                .update(cx, |modal, cx| {
                    if modal.search_generation == generation
                        && modal.tab == tab
                        && modal.search.read(cx).value().trim() == query
                    {
                        modal.replace_rows(search_rows(tab, indices));
                        cx.notify();
                    }
                })
                .ok();
        }));
    }

    fn render_tab(&self, tab: Tab, font_size: Pixels, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.tab == tab;
        div()
            .id(("help-tab", tab.index()))
            .px(px(0.7 * theme::REM))
            .py(px(0.25 * theme::REM))
            .text_size(font_size * 0.8)
            .line_height(font_size)
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if active { rgb(0xe6ebee) } else { rgb(0x8a9196) })
            .when(active, |pill| pill.bg(rgb(0x383d46)))
            .rounded(px(0.35 * theme::REM))
            .cursor(CursorStyle::Arrow)
            .when(!active, |pill| {
                pill.hover(|style| style.bg(rgba(0xffffff0a)).text_color(rgb(0xcdd4d7)))
            })
            .on_click(cx.listener(move |modal, _, window, cx| {
                modal.set_tab(tab, window, cx);
            }))
            .child(tab.label())
    }

    fn render_category_header(
        &self,
        tab: Tab,
        category: &str,
        count: usize,
        category_index: usize,
        font_size: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let open = self.is_open(tab, category);
        let key = section_key(tab, category);
        let transition = self
            .chevron_animation
            .as_ref()
            .filter(|animation| {
                animation.key == key && animation.started.elapsed() < Duration::from_millis(120)
            })
            .map(|animation| (animation.opening, animation.serial));
        let chevron = if let Some((opening, serial)) = transition {
            svg()
                .path(icon::CHEVRON)
                .size(px(0.8 * theme::REM))
                .text_color(rgb(0x6b7280))
                .with_animation(
                    SharedString::from(format!("help-chevron-{serial}")),
                    Animation::new(Duration::from_millis(120)).with_easing(gpui::ease_in_out),
                    move |chevron, progress| {
                        let angle = if opening {
                            90.0 * progress
                        } else {
                            90.0 * (1.0 - progress)
                        };
                        chevron.with_transformation(Transformation::rotate(radians(
                            angle.to_radians(),
                        )))
                    },
                )
                .into_any_element()
        } else {
            svg()
                .path(icon::CHEVRON)
                .size(px(0.8 * theme::REM))
                .text_color(rgb(0x6b7280))
                .with_transformation(Transformation::rotate(radians(
                    if open { 90.0_f32 } else { 0.0_f32 }.to_radians(),
                )))
                .into_any_element()
        };
        div()
            .id(("help-category", category_index))
            .w_full()
            .py(px(0.6 * theme::REM))
            .flex()
            .items_center()
            .text_size(font_size * 0.72)
            .line_height(font_size * 0.9)
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0x9aa1a8))
            .cursor(CursorStyle::Arrow)
            .hover(|style| style.text_color(rgb(0xcdd4d7)))
            .on_click(cx.listener(move |modal, _, _, cx| {
                modal.toggle_section(key.clone(), cx);
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(0.65 * theme::REM))
                    .child(
                        div()
                            .size(px(0.8 * theme::REM))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(chevron),
                    )
                    .child(category.to_uppercase()),
            )
            .child(
                div()
                    .ml_auto()
                    .text_size(font_size * 0.7)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x5c636e))
                    .child(count.to_string()),
            )
    }

    fn render_overview_entry(&self, entry: &OverviewEntry, font_size: Pixels) -> impl IntoElement {
        div()
            .px(px(0.2 * theme::REM))
            .pt(px(0.5 * theme::REM))
            .pb(px(0.7 * theme::REM))
            .child(
                div()
                    .mb(px(0.2 * theme::REM))
                    .text_size(font_size * 0.9)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xe6ebee))
                    .child(entry.title),
            )
            .child(entry_description(entry.description, font_size))
            .child(help_example_block(
                entry
                    .examples
                    .iter()
                    .map(|line| (self.example_input(line.input), line.result)),
                font_size,
            ))
    }

    fn render_catalog_entry(
        &self,
        title: &str,
        description: &str,
        examples: &[CatalogExample],
        font_size: Pixels,
    ) -> impl IntoElement {
        div()
            .px(px(0.2 * theme::REM))
            .pt(px(0.5 * theme::REM))
            .pb(px(0.7 * theme::REM))
            .child(
                div()
                    .mb(px(0.35 * theme::REM))
                    .font_family(self.mono_family())
                    .text_size(font_size * 0.85)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x90bee3))
                    .child(title.replace("  ·  ", " · ")),
            )
            .child(entry_description(description, font_size))
            .child(help_example_block(
                examples
                    .iter()
                    .map(|line| (self.example_input(&line.input), Some(line.result.as_str()))),
                font_size,
            ))
    }

    fn render_unit(&self, unit: &CatalogUnit, font_size: Pixels) -> impl IntoElement {
        div()
            .w_full()
            .px(px(0.2 * theme::REM))
            .py(px(0.3 * theme::REM))
            .flex()
            .items_baseline()
            .child(
                div()
                    .min_w_0()
                    .flex_shrink()
                    .flex()
                    .items_baseline()
                    .gap(px(0.6 * theme::REM))
                    .child(
                        div()
                            .flex_none()
                            .min_w(px(3.2 * theme::REM))
                            .font_family(self.mono_family())
                            .text_size(font_size * 0.82)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xdec4ab))
                            .child(unit.symbol.clone()),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_shrink()
                            .whitespace_nowrap()
                            .text_size(font_size * 0.82)
                            .text_color(rgb(0xcdd4d7))
                            .child(unit.name.clone()),
                    )
                    .children(unit.si_prefixes.then(|| {
                        let hint = SharedString::from(format!(
                            "Takes any SI prefix: write n{} or nano{}",
                            unit.symbol, unit.name
                        ));
                        div()
                            .id(SharedString::from(format!(
                                "help-prefix-{}-{}",
                                unit.symbol, unit.name
                            )))
                            .flex_none()
                            .px(px(0.3 * theme::REM))
                            .border_1()
                            .border_color(rgb(0x33434f))
                            .rounded(px(3.0))
                            .text_size(font_size * 0.62)
                            .line_height(font_size * 0.93)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x90bee3))
                            .cursor(CursorStyle::Arrow)
                            .whitespace_nowrap()
                            .tooltip(move |_, cx| cx.new(|_| TextTooltip::new(hint.clone())).into())
                            .child("SI Prefix")
                    })),
            )
            .children((!unit.aliases.is_empty()).then(|| {
                div()
                    .ml_auto()
                    .pl(px(0.8 * theme::REM))
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .text_right()
                    .text_size(font_size * 0.75)
                    .text_color(rgb(0x6b7280))
                    .child(unit.aliases.join(", "))
            }))
    }

    fn render_function(&self, function: &CatalogFunction, font_size: Pixels) -> impl IntoElement {
        self.render_catalog_entry(
            &function.signature,
            &function.description,
            &function.examples,
            font_size,
        )
    }

    fn render_constant(&self, constant: &CatalogConstant, font_size: Pixels) -> impl IntoElement {
        self.render_catalog_entry(
            &constant.name,
            &constant.description,
            &constant.examples,
            font_size,
        )
    }

    fn mono_family(&self) -> SharedString {
        self.mono_family.clone()
    }

    fn example_input(&self, input: &str) -> Entity<SelectableCodeLine> {
        self.example_inputs
            .get(input)
            .expect("every help example has a selectable input")
            .clone()
    }

    fn render_row(&self, row_index: usize, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let font_size = help_font_size(window);
        let Some(row) = self.rows.get(row_index).cloned() else {
            return div().into_any_element();
        };
        match row {
            HelpRow::Category {
                tab,
                name,
                count,
                index,
                border_top,
            } => div()
                .when(border_top, |section| {
                    section.border_t_1().border_color(rgb(0x2b2e36))
                })
                .child(self.render_category_header(tab, &name, count, index, font_size, cx))
                .into_any_element(),
            HelpRow::OverviewEntry(index) => overview_entry(index)
                .map(|entry| {
                    self.render_overview_entry(entry, font_size)
                        .into_any_element()
                })
                .unwrap_or_else(|| div().into_any_element()),
            HelpRow::Function(index) => self
                .catalog
                .functions
                .get(index)
                .map(|function| self.render_function(function, font_size).into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            HelpRow::Constant(index) => self
                .catalog
                .constants
                .get(index)
                .map(|constant| self.render_constant(constant, font_size).into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            HelpRow::Unit(index) => self
                .catalog
                .units
                .get(index)
                .map(|unit| self.render_unit(unit, font_size).into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            HelpRow::SearchUnit(index) => self
                .searchable_units
                .get(index)
                .map(|unit| self.render_unit(unit, font_size).into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            HelpRow::Gap => div().h(px(0.5 * theme::REM)).flex_none().into_any_element(),
            HelpRow::Empty => empty_state(font_size).into_any_element(),
        }
    }
}

impl Render for HelpModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let font_size = help_font_size(window);
        let content = modal_body("help-body").child(
            list(
                self.list_state.clone(),
                cx.processor(|modal, index: usize, window, cx| modal.render_row(index, window, cx)),
            )
            .size_full(),
        );

        overlay("help-overlay")
            .capture_key_down(cx.listener(Self::handle_overlay_key_down))
            .on_action(cx.listener(|modal, _: &Left, window, cx| modal.step_tab(-1, window, cx)))
            .on_action(cx.listener(|modal, _: &Right, window, cx| modal.step_tab(1, window, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|modal, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    modal.close(cx);
                }),
            )
            .child(
                panel("help-panel", ModalSize::new(px(640.0), px(760.0)))
                    .text_size(font_size * 0.85)
                    .line_height(font_size * 1.15)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|modal, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            modal.version.update(cx, VersionButton::close_menu);
                        }),
                    )
                    .child(
                        header()
                            .child(
                                div()
                                    .flex()
                                    .gap(px(0.35 * theme::REM))
                                    .child(self.render_tab(Tab::Overview, font_size, cx))
                                    .child(self.render_tab(Tab::Functions, font_size, cx))
                                    .child(self.render_tab(Tab::Constants, font_size, cx))
                                    .child(self.render_tab(Tab::Units, font_size, cx)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(0.35 * theme::REM))
                                    .child(self.version.clone())
                                    .child(close_button("help-close").on_click(cx.listener(
                                        |modal, _, _, cx| {
                                            modal.close(cx);
                                        },
                                    ))),
                            ),
                    )
                    .child(self.search.clone())
                    .child(content),
            )
    }
}

impl Focusable for HelpModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle()
    }
}

fn build_example_inputs(
    catalog: &Catalog,
    mono_family: SharedString,
    cx: &mut Context<HelpModal>,
) -> HashMap<String, Entity<SelectableCodeLine>> {
    let mut inputs: HashSet<String> = OVERVIEW
        .iter()
        .flat_map(|category| category.entries)
        .flat_map(|entry| entry.examples)
        .map(|line| line.input.to_string())
        .collect();
    inputs.extend(
        catalog
            .functions
            .iter()
            .flat_map(|function| &function.examples)
            .chain(
                catalog
                    .constants
                    .iter()
                    .flat_map(|constant| &constant.examples),
            )
            .map(|line| line.input.clone()),
    );

    inputs
        .into_iter()
        .map(|input| {
            let content = SharedString::from(input.clone());
            let family = mono_family.clone();
            let entity = cx.new(|cx| SelectableCodeLine::new(content, family, cx));
            (input, entity)
        })
        .collect()
}

fn search_field_metrics() -> FieldMetrics {
    FieldMetrics {
        padding_x: px(0.7 * theme::REM),
        padding_y: px(0.45 * theme::REM),
        caret_height: px(0.95 * theme::REM),
        selection: theme::selection(),
        selection_text: None,
        placeholder: rgb(0x6b7280),
        centered: false,
    }
}

fn search_field_chrome(field: Stateful<Div>, state: FieldState) -> Stateful<Div> {
    field
        .overflow_x_scroll()
        .mx(px(0.9 * theme::REM))
        .mb(px(0.6 * theme::REM))
        .bg(rgba(0x00000040))
        .border_1()
        .border_color(if state.focused {
            rgb(0x4b5263)
        } else {
            rgb(0x33363f)
        })
        .rounded(px(0.4 * theme::REM))
        .text_color(rgb(0xcdd4d7))
        .cursor(CursorStyle::IBeam)
}

fn help_font_size(window: &Window) -> Pixels {
    theme::font_size_for_window(window.viewport_size().width)
}

fn entry_description(text: &str, font_size: Pixels) -> impl IntoElement {
    div()
        .mb(px(0.5 * theme::REM))
        .text_size(font_size * 0.82)
        .line_height(font_size * 1.23)
        .font_weight(FontWeight::NORMAL)
        .text_color(rgb(0xa9b0b6))
        .child(text.to_string())
}

fn empty_state(font_size: Pixels) -> impl IntoElement {
    div()
        .px(px(0.5 * theme::REM))
        .py(px(1.5 * theme::REM))
        .text_center()
        .text_size(font_size * 0.85)
        .text_color(rgb(0x6b7280))
        .child("No matches.")
}

fn help_example_block<'a>(
    lines: impl IntoIterator<Item = (Entity<SelectableCodeLine>, Option<&'a str>)>,
    font_size: Pixels,
) -> impl IntoElement {
    let lines: Vec<_> = lines.into_iter().collect();
    let id = lines
        .first()
        .expect("a help example block has at least one line")
        .0
        .entity_id();
    code_block(
        ("help-example", id),
        lines.into_iter().map(|(input, result)| match result {
            Some(result) => CodeBlockLine::with_result(input, result.to_string()),
            None => CodeBlockLine::new(input),
        }),
        font_size,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_keeps_its_shape() {
        assert_eq!(OVERVIEW.len(), 7);
        assert_eq!(
            OVERVIEW
                .iter()
                .map(|category| category.entries.len())
                .sum::<usize>(),
            22
        );
    }
}
