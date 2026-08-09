//! The searchable help dialog view.
//!
//! Catalog entries come from `siffra::catalog`; the introductory overview is
//! authored in this module.

use crate::fuzzy;
use crate::text::{Left, Right};
use crate::ui::assets::icon;
use crate::ui::code_block::{CodeBlockLine, CodeLineActivated, SelectableCodeLine, code_block};
use crate::ui::modal::{ModalSize, body as modal_body, close_button, header, overlay, panel};
use crate::ui::single_line_input::{FieldMetrics, FieldState, SingleLineInput, ValueChanged};
use crate::ui::theme;
use crate::ui::tooltip::TextTooltip;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Functions,
    Constants,
    Units,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Overview, Tab::Functions, Tab::Constants, Tab::Units];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Functions => "Functions",
            Self::Constants => "Constants",
            Self::Units => "Units",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Functions => "functions",
            Self::Constants => "constants",
            Self::Units => "units",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Functions => 1,
            Self::Constants => 2,
            Self::Units => 3,
        }
    }
}

#[derive(Clone, Copy)]
struct ExampleLine {
    input: &'static str,
    result: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct OverviewEntry {
    title: &'static str,
    description: &'static str,
    keywords: &'static [&'static str],
    examples: &'static [ExampleLine],
}

#[derive(Clone, Copy)]
struct OverviewCategory {
    name: &'static str,
    entries: &'static [OverviewEntry],
}

macro_rules! line {
    ($input:literal) => {
        ExampleLine {
            input: $input,
            result: None,
        }
    };
    ($input:literal => $result:literal) => {
        ExampleLine {
            input: $input,
            result: Some($result),
        }
    };
}

const OVERVIEW: &[OverviewCategory] = &[
    OverviewCategory {
        name: "Arithmetic",
        entries: &[
            OverviewEntry {
                title: "Addition & subtraction",
                description: "Add and subtract with the usual symbols, or spell them out as “plus” and “minus”.",
                keywords: &["add", "subtract", "plus", "minus", "sum", "+", "-"],
                examples: &[line!("2 + 3" => "5"), line!("10 minus 4" => "6")],
            },
            OverviewEntry {
                title: "Multiplication & division",
                description: "Multiply and divide with *, /, or words like “times”, “over”, and “divided by”.",
                keywords: &[
                    "multiply",
                    "divide",
                    "times",
                    "over",
                    "divided by",
                    "product",
                    "*",
                    "/",
                ],
                examples: &[
                    line!("6 * 7" => "42"),
                    line!("20 over 4" => "5"),
                    line!("100 divided by 8" => "12.5"),
                ],
            },
            OverviewEntry {
                title: "Powers & grouping",
                description: "Raise to a power with ^, and group sub-expressions with parentheses.",
                keywords: &["power", "exponent", "parentheses", "brackets", "^"],
                examples: &[line!("2 ^ 10" => "1,024"), line!("(3 + 4) * 2" => "14")],
            },
            OverviewEntry {
                title: "Variables",
                description: "Name a value with “=” and reuse it on later lines.",
                keywords: &["variable", "assign", "let", "name", "="],
                examples: &[line!("price = 40" => "40"), line!("price * 3" => "120")],
            },
        ],
    },
    OverviewCategory {
        name: "Conversions",
        entries: &[
            OverviewEntry {
                title: "Convert between units",
                description: "Use “in”, “to”, or “as” to convert a value into another compatible unit.",
                keywords: &["convert", "in", "to", "as", "into", "unit"],
                examples: &[
                    line!("5 km in miles" => "3.106856 mi"),
                    line!("2 hours in minutes" => "120 min"),
                ],
            },
            OverviewEntry {
                title: "Temperature & data",
                description: "Conversions work across every dimension — temperature scales, data sizes, currencies, and more.",
                keywords: &["temperature", "celsius", "fahrenheit", "data", "gb", "mb"],
                examples: &[
                    line!("100 F in C" => "37.777778 °C"),
                    line!("1 GB in MB" => "1,000 MB"),
                ],
            },
            OverviewEntry {
                title: "Unit analysis",
                description: "Multiply, divide, and raise units to powers — Siffra tracks the dimensions and keeps the answer's units correct.",
                keywords: &[
                    "dimensional",
                    "dimension",
                    "analysis",
                    "area",
                    "volume",
                    "per",
                    "compound",
                    "bracket",
                    "m^2",
                ],
                examples: &[
                    line!("10 m * 5 m" => "50 m^2"),
                    line!("100 km / 2 h" => "50 km/h"),
                    line!("(3 m)^2" => "9 m^2"),
                    line!("5 N * 2 m" => "10 N*m"),
                    line!("5 [kg * m / s^2] in N" => "5 N"),
                ],
            },
        ],
    },
    OverviewCategory {
        name: "Percentages",
        entries: &[
            OverviewEntry {
                title: "Take, add, or remove a percentage",
                description: "“of” takes a percentage; “on” adds it; “off” removes it. Works with any unit or currency.",
                keywords: &["percent", "of", "on", "off", "discount", "tax", "tip", "%"],
                examples: &[
                    line!("20% of $10" => "2 USD"),
                    line!("5% on $30" => "31.5 USD"),
                    line!("6% off 40 EUR" => "37.6 EUR"),
                ],
            },
            OverviewEntry {
                title: "One value as a percentage of another",
                description: "“X as a % of / on / off Y” expresses X relative to Y as a percentage.",
                keywords: &["percent", "relative", "as a", "ratio", "share", "%"],
                examples: &[
                    line!("$50 as a % of $100" => "50 %"),
                    line!("$70 as a % on $20" => "250 %"),
                    line!("$20 as a % off $70" => "71.428571 %"),
                ],
            },
            OverviewEntry {
                title: "Find the value behind a percentage",
                description: "“X% of / on / off what is Y” works backwards from a percentage and a result to the original value.",
                keywords: &["percent", "reverse", "what", "original", "backwards", "%"],
                examples: &[
                    line!("5% of what is 6 EUR" => "120 EUR"),
                    line!("5% on what is 6 EUR" => "5.714286 EUR"),
                    line!("5% off what is 6 EUR" => "6.315789 EUR"),
                ],
            },
        ],
    },
    OverviewCategory {
        name: "Molar Mass Conversions",
        entries: &[
            OverviewEntry {
                title: "Grams to moles of a compound",
                description: "Tag a mass with a chemical formula and convert it to an amount of substance — Siffra looks up the molar mass from the periodic table.",
                keywords: &[
                    "stoichiometry",
                    "chemistry",
                    "compound",
                    "formula",
                    "molar mass",
                    "molecular weight",
                    "mole",
                    "mol",
                    "O2",
                    "NaCl",
                ],
                examples: &[
                    line!("5 g O2 in mol O2" => "0.15626 mol O2"),
                    line!("10 g NaCl in mol NaCl" => "0.171116 mol NaCl"),
                ],
            },
            OverviewEntry {
                title: "Moles to grams of a compound",
                description: "Go the other way to get a mass. Parentheses and subscripts in formulas like Ca(NO3)2 are understood.",
                keywords: &[
                    "stoichiometry",
                    "chemistry",
                    "compound",
                    "formula",
                    "molar mass",
                    "mole",
                    "mol",
                    "grams",
                    "Ca(NO3)2",
                    "H2O",
                ],
                examples: &[
                    line!("5 mol Ca(NO3)2 into g" => "820.44 g"),
                    line!("2 mol H2O in g" => "36.03 g"),
                ],
            },
            OverviewEntry {
                title: "Bracketed compound units",
                description: "Wrap a compound unit in [ ] when you want spaces or need it inside a larger expression.",
                keywords: &[
                    "stoichiometry",
                    "chemistry",
                    "compound",
                    "brackets",
                    "molar",
                    "H2O",
                ],
                examples: &[line!("18 [g H2O] as [mol H2O]" => "0.999167 mol H2O")],
            },
        ],
    },
    OverviewCategory {
        name: "Aggregates & recall",
        entries: &[
            OverviewEntry {
                title: "Sum a column",
                description: "“total” (or “sum”) adds up the results above it, back to the last blank line.",
                keywords: &["total", "sum", "column", "add", "aggregate"],
                examples: &[
                    line!("10" => "10"),
                    line!("20" => "20"),
                    line!("30" => "30"),
                    line!("total" => "60"),
                ],
            },
            OverviewEntry {
                title: "Reuse the previous answer",
                description: "“prev” (also “ans”, “previous”, or “that”) refers to the result on the line above.",
                keywords: &["prev", "ans", "previous", "that", "recall", "answer"],
                examples: &[line!("5 * 5" => "25"), line!("prev + 10" => "35")],
            },
        ],
    },
    OverviewCategory {
        name: "Naming & notes",
        entries: &[
            OverviewEntry {
                title: "Headings",
                description: "Start any line with # to make it a heading. Headings are shown larger and produce no result, so they can title a calculation or break a long one into sections. To name the file itself, click its name in the title bar.",
                keywords: &[
                    "header", "heading", "title", "section", "name", "file", "rename", "#",
                ],
                examples: &[
                    line!("# Trip budget"),
                    line!("flights = 450 USD" => "450 USD"),
                    line!("## Per person"),
                    line!("flights / 2" => "225 USD"),
                ],
            },
            OverviewEntry {
                title: "Comments",
                description: "Anything after // is ignored, as is a /* … */ block, so you can annotate a line without changing its result.",
                keywords: &["comment", "note", "ignore", "//", "/*"],
                examples: &[
                    line!("60 * 8 // hours in a work week" => "480"),
                    line!("/* scratch space */"),
                ],
            },
        ],
    },
];

pub struct CloseHelp;

#[derive(Clone, PartialEq, Eq)]
enum HelpRow {
    Category {
        tab: Tab,
        name: SharedString,
        count: usize,
        index: usize,
        border_top: bool,
    },
    OverviewEntry(usize),
    Function(usize),
    Constant(usize),
    Unit(usize),
    SearchUnit(usize),
    Gap,
    Empty,
}

struct SearchIndex {
    overview: Vec<Vec<String>>,
    functions: Vec<Vec<String>>,
    constants: Vec<Vec<String>>,
    units: Vec<Vec<String>>,
}

impl SearchIndex {
    fn documents(&self, tab: Tab) -> &[Vec<String>] {
        match tab {
            Tab::Overview => &self.overview,
            Tab::Functions => &self.functions,
            Tab::Constants => &self.constants,
            Tab::Units => &self.units,
        }
    }
}

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
}

impl EventEmitter<CloseHelp> for HelpModal {}

impl HelpModal {
    pub fn new(mono_family: SharedString, cx: &mut Context<Self>) -> Self {
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
        }
    }

    pub fn open(&mut self, cx: &mut Context<Self>) {
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

    fn section_key(tab: Tab, category: &str) -> String {
        format!("{}:{category}", tab.key())
    }

    fn is_open(&self, tab: Tab, category: &str) -> bool {
        self.open_sections
            .contains(&Self::section_key(tab, category))
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
        let key = Self::section_key(tab, category);
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
            // An empty search field passes the arrows on rather than moving a
            // caret it has no text for, which is what walks the tabs.
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
                        cx.listener(|_, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
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
                            .child(close_button("help-close").on_click(cx.listener(
                                |modal, _, _, cx| {
                                    modal.close(cx);
                                },
                            ))),
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

fn overview_entry(index: usize) -> Option<&'static OverviewEntry> {
    OVERVIEW
        .iter()
        .flat_map(|category| category.entries.iter())
        .nth(index)
}

fn non_search_rows(tab: Tab, catalog: &Catalog, open_sections: &HashSet<String>) -> Vec<HelpRow> {
    let mut rows = Vec::new();
    match tab {
        Tab::Overview => {
            let mut entry_index = 0;
            for (category_index, category) in OVERVIEW.iter().enumerate() {
                rows.push(HelpRow::Category {
                    tab,
                    name: category.name.into(),
                    count: category.entries.len(),
                    index: category_index,
                    border_top: category_index > 0,
                });
                if open_sections.contains(&HelpModal::section_key(tab, category.name)) {
                    rows.extend(
                        (entry_index..entry_index + category.entries.len())
                            .map(HelpRow::OverviewEntry),
                    );
                    rows.push(HelpRow::Gap);
                }
                entry_index += category.entries.len();
            }
        }
        Tab::Functions => {
            for (category_index, (name, indices)) in
                indices_by_category(&catalog.functions, |function| function.category.as_str())
                    .into_iter()
                    .enumerate()
            {
                rows.push(HelpRow::Category {
                    tab,
                    name: SharedString::from(name.to_string()),
                    count: indices.len(),
                    index: category_index,
                    border_top: category_index > 0,
                });
                if open_sections.contains(&HelpModal::section_key(tab, name)) {
                    rows.extend(indices.into_iter().map(HelpRow::Function));
                    rows.push(HelpRow::Gap);
                }
            }
        }
        Tab::Constants => {
            rows.extend((0..catalog.constants.len()).map(HelpRow::Constant));
            rows.push(HelpRow::Gap);
        }
        Tab::Units => {
            for (category_index, (name, indices)) in
                indices_by_category(&catalog.units, |unit| unit.category.as_str())
                    .into_iter()
                    .enumerate()
            {
                rows.push(HelpRow::Category {
                    tab,
                    name: SharedString::from(name.to_string()),
                    count: indices.len(),
                    index: category_index,
                    border_top: category_index > 0,
                });
                if open_sections.contains(&HelpModal::section_key(tab, name)) {
                    rows.extend(indices.into_iter().map(HelpRow::Unit));
                    rows.push(HelpRow::Gap);
                }
            }
        }
    }
    rows
}

fn search_rows(tab: Tab, indices: Vec<usize>) -> Vec<HelpRow> {
    if indices.is_empty() {
        return vec![HelpRow::Empty];
    }
    let mut rows: Vec<_> = indices
        .into_iter()
        .map(|index| match tab {
            Tab::Overview => HelpRow::OverviewEntry(index),
            Tab::Functions => HelpRow::Function(index),
            Tab::Constants => HelpRow::Constant(index),
            Tab::Units => HelpRow::SearchUnit(index),
        })
        .collect();
    rows.push(HelpRow::Gap);
    rows
}

fn indices_by_category<'a, T>(
    items: &'a [T],
    category: impl Fn(&'a T) -> &'a str,
) -> Vec<(&'a str, Vec<usize>)> {
    let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let name = category(item);
        if let Some((_, group)) = groups.iter_mut().find(|(current, _)| *current == name) {
            group.push(index);
        } else {
            groups.push((name, vec![index]));
        }
    }
    groups
}

fn build_search_index(catalog: &Catalog, searchable_units: &[CatalogUnit]) -> SearchIndex {
    fn normalized(keys: impl IntoIterator<Item = String>) -> Vec<String> {
        keys.into_iter().map(|key| key.to_lowercase()).collect()
    }

    SearchIndex {
        overview: OVERVIEW
            .iter()
            .flat_map(|category| category.entries)
            .map(|entry| {
                normalized(
                    [entry.title.to_string(), entry.description.to_string()]
                        .into_iter()
                        .chain(entry.keywords.iter().map(|keyword| (*keyword).to_string()))
                        .chain(
                            entry
                                .examples
                                .iter()
                                .map(|example| example.input.to_string()),
                        ),
                )
            })
            .collect(),
        functions: catalog
            .functions
            .iter()
            .map(|function| {
                normalized([
                    function.name.clone(),
                    function.signature.clone(),
                    function.description.clone(),
                ])
            })
            .collect(),
        constants: catalog
            .constants
            .iter()
            .map(|constant| normalized([constant.name.clone(), constant.description.clone()]))
            .collect(),
        units: searchable_units
            .iter()
            .map(|unit| {
                normalized(
                    [unit.symbol.clone(), unit.name.clone()]
                        .into_iter()
                        .chain(unit.aliases.iter().cloned()),
                )
            })
            .collect(),
    }
}

fn scored_document_indices(query: &str, documents: &[Vec<String>]) -> Vec<usize> {
    let query = query.to_lowercase();
    let mut scored: Vec<(usize, i32)> = documents
        .iter()
        .enumerate()
        .filter_map(|(order, keys)| {
            let score = keys
                .iter()
                .filter_map(|key| fuzzy::score_lowercase(&query, key))
                .max()?;
            Some((order, score))
        })
        .collect();
    scored.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    scored.into_iter().map(|(index, _)| index).collect()
}

fn default_open_sections(catalog: &Catalog) -> HashSet<String> {
    OVERVIEW
        .iter()
        .map(|category| HelpModal::section_key(Tab::Overview, category.name))
        .chain(
            catalog
                .functions
                .iter()
                .map(|function| HelpModal::section_key(Tab::Functions, &function.category)),
        )
        .collect()
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

fn changed_rows(old: &[HelpRow], new: &[HelpRow]) -> (std::ops::Range<usize>, usize) {
    let prefix = old
        .iter()
        .zip(new)
        .take_while(|(old, new)| old == new)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(old, new)| old == new)
        .count();
    (prefix..old.len() - suffix, new.len() - prefix - suffix)
}

fn searchable_units(catalog: &Catalog) -> Vec<CatalogUnit> {
    let named: HashSet<&str> = catalog
        .units
        .iter()
        .map(|unit| unit.symbol.as_str())
        .collect();
    let mut expanded = Vec::new();

    for unit in &catalog.units {
        expanded.push(CatalogUnit {
            category: unit.category.clone(),
            name: unit.name.clone(),
            symbol: unit.symbol.clone(),
            aliases: unit.aliases.clone(),
            si_prefixes: unit.si_prefixes,
        });
        if !unit.si_prefixes {
            continue;
        }

        for prefix in &catalog.prefixes {
            let symbol = format!("{}{}", prefix.symbol, unit.symbol);
            if named.contains(symbol.as_str()) {
                continue;
            }
            expanded.push(CatalogUnit {
                category: unit.category.clone(),
                name: format!("{}{}", prefix.name, unit.name),
                symbol,
                aliases: unit
                    .aliases
                    .iter()
                    .filter(|alias| {
                        alias.len() >= 3
                            && alias
                                .chars()
                                .all(|character| character.is_ascii_lowercase())
                    })
                    .map(|alias| format!("{}{}", prefix.name, alias))
                    .collect(),
                si_prefixes: false,
            });
        }
    }

    expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_matches_legacy_shape() {
        assert_eq!(OVERVIEW.len(), 6);
        assert_eq!(
            OVERVIEW
                .iter()
                .map(|category| category.entries.len())
                .sum::<usize>(),
            17
        );
    }

    #[test]
    fn prefix_expansion_makes_spelled_and_symbol_forms_searchable() {
        let catalog = catalog();
        let units = searchable_units(&catalog);
        assert!(units.iter().any(|unit| unit.symbol == "nF"));
        assert!(units.iter().any(|unit| unit.name == "nanofarad"));
    }

    #[test]
    fn background_search_indices_keep_catalog_identity() {
        let catalog = catalog();
        let units = searchable_units(&catalog);
        let index = build_search_index(&catalog, &units);

        let sqrt = catalog
            .functions
            .iter()
            .position(|function| function.name == "sqrt")
            .unwrap();
        assert_eq!(
            scored_document_indices("sqrt", index.documents(Tab::Functions))[0],
            sqrt
        );

        let nanofarad = units.iter().position(|unit| unit.symbol == "nF").unwrap();
        assert_eq!(
            scored_document_indices("nF", index.documents(Tab::Units))[0],
            nanofarad
        );
    }

    #[test]
    fn virtual_rows_preserve_all_open_overview_entries() {
        let catalog = catalog();
        let rows = non_search_rows(Tab::Overview, &catalog, &default_open_sections(&catalog));
        assert_eq!(
            rows.iter()
                .filter(|row| matches!(row, HelpRow::OverviewEntry(_)))
                .count(),
            17
        );
    }

    #[test]
    fn collapsing_a_section_splices_only_its_contents() {
        let catalog = catalog();
        let open = default_open_sections(&catalog);
        let before = non_search_rows(Tab::Overview, &catalog, &open);
        let mut closed = open;
        closed.remove(&HelpModal::section_key(Tab::Overview, OVERVIEW[0].name));
        let after = non_search_rows(Tab::Overview, &catalog, &closed);

        assert_eq!(changed_rows(&before, &after), (1..6, 0));
    }
}
