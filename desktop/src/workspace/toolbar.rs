//! The workspace toolbar: what the calculation is called, and
//! the buttons beside it.
//!
//! The window's own title bar is drawn transparent, so this stands in for it.
//! That means it is also the window's drag handle — everywhere except the name,
//! which is a field rather than somewhere to take hold of the window.
//!
//! The bar owns two things outright: the name, which it edits and hands back
//! when the field is committed, and the pin, which is nothing but a window
//! level. What the other buttons open belongs to the workspace, so they are
//! reported as events.

use crate::platform;
use crate::ui::UI_FONT;
use crate::ui::assets::icon;
use crate::ui::single_line_input::{FieldMetrics, FieldState, SingleLineInput, ValueChanged};
use crate::ui::theme;
use crate::ui::tooltip::TextTooltip;
use gpui::{
    AnyView, App, AppContext, ClickEvent, Context, CursorStyle, Div, Entity, EventEmitter,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseUpEvent, ParentElement, Pixels, Point, Render, SharedString, Stateful,
    StatefulInteractiveElement, Styled, Subscription, Window, WindowControlArea, div,
    prelude::FluentBuilder, px, svg,
};

/// How far the window may travel under a press before it counts as having been
/// dragged rather than pressed.
const DRAG_SLOP: f64 = 2.0;

/// The toolbar uses the interface font rather than the editor's monospace font.
const UI_FONT_FAMILY: &str = UI_FONT;

#[cfg(target_os = "macos")]
const MOD_KEY: &str = "⌘";
#[cfg(not(target_os = "macos"))]
const MOD_KEY: &str = "Ctrl ";

pub enum ToolbarEvent {
    NewDocument,
    OpenSearch,
    ToggleHelp,
    Rename(String),
    NameEditingStarted,
    NameEditingFinished,
}

pub struct Toolbar {
    /// What the open calculation is called on screen: the name the user gave it,
    /// or the date it was created while it has none.
    title: SharedString,
    /// The name field. It holds what the user is typing; while the field is idle
    /// it is kept in step with `title`
    name: Entity<SingleLineInput>,
    editing: bool,
    /// The name last given to the window, so the platform is only troubled when
    /// it actually changes.
    window_title: Option<SharedString>,
    /// Whether the window is being held above the others.
    pinned: bool,
    help_open: bool,
    /// Held only while the name is being edited: whatever the user clicks next
    /// commits what they typed, exactly as leaving the field does.
    blur_subscription: Option<Subscription>,
    /// The press now in progress, if it began in the bar.
    press: Option<Press>,
    name_hovered: bool,
    window_movable: bool,
    _subscriptions: Vec<Subscription>,
}

/// A press in the bar, from the moment it begins. The bar is the window's drag
/// handle, so a press that takes the window with it was a drag of the window
/// rather than a press on whatever it started over — which is only knowable once
/// the press is over, and only by what became of the window.
#[derive(Clone, Copy)]
struct Press {
    /// Where the window stood when the press began.
    window_origin: Point<Pixels>,
    /// Whether it began over the name, which is the only part of the bar that
    /// acts on the press itself rather than on a click.
    on_name: bool,
}

impl EventEmitter<ToolbarEvent> for Toolbar {}

impl Toolbar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| {
            let mut field = SingleLineInput::new(
                "document-title",
                "Untitled",
                name_field_metrics(),
                name_field_chrome,
                cx,
            );
            // Until it is clicked the field is only there to be read, so a press
            // on it starts a rename rather than dropping a caret into the name.
            field.set_read_only(true, cx);
            field
        });
        let subscriptions = vec![cx.subscribe(&name, |_, _, _: &ValueChanged, cx| {
            cx.notify();
        })];

        Self {
            title: SharedString::default(),
            name,
            editing: false,
            window_title: None,
            pinned: false,
            help_open: false,
            blur_subscription: None,
            press: None,
            name_hovered: false,
            window_movable: true,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_title(&mut self, title: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.title = title.into();
        // Do not overwrite a draft while the name field is being edited.
        if !self.editing {
            let title = self.title.clone();
            self.name.update(cx, |name, cx| name.set_value(title, cx));
        }
        cx.notify();
    }

    pub fn set_help_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.help_open = open;
        cx.notify();
    }

    /// Moves focus into the name field with the current name selected, so typing
    /// replaces it. Also reachable from the workspace's ⌘S.
    pub fn start_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing {
            return;
        }
        self.editing = true;
        let title = self.title.clone();
        let handle = self.name.update(cx, |name, cx| {
            name.set_read_only(false, cx);
            name.set_value(title, cx);
            name.select_all(cx);
            name.focus_handle()
        });
        handle.focus(window);
        self.blur_subscription = Some(cx.on_blur(&handle, window, |toolbar, window, cx| {
            toolbar.commit(window, cx);
        }));
        self.apply_window_movable(window);
        cx.emit(ToolbarEvent::NameEditingStarted);
        cx.notify();
    }

    /// Hands the name back to the workspace, unless nothing about it changed.
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        self.blur_subscription = None;
        self.apply_window_movable(window);
        let draft = self.name.read(cx).value().to_string();
        self.name
            .update(cx, |name, cx| name.set_read_only(true, cx));

        if draft.trim() == self.title.as_ref() {
            // Restore the canonical title when trimming made this a no-op.
            let title = self.title.clone();
            self.name.update(cx, |name, cx| name.set_value(title, cx));
        } else {
            cx.emit(ToolbarEvent::Rename(draft));
        }
        cx.notify();
    }

    /// Abandons the edit; the field falls back to the stored name.
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        self.blur_subscription = None;
        self.apply_window_movable(window);
        let title = self.title.clone();
        self.name.update(cx, |name, cx| {
            name.set_read_only(true, cx);
            name.set_value(title, cx);
        });
        cx.notify();
    }

    fn on_name_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editing {
            return;
        }
        match event.keystroke.key.as_str() {
            "enter" => {
                cx.stop_propagation();
                self.commit(window, cx);
                window.blur();
                cx.emit(ToolbarEvent::NameEditingFinished);
            }
            "escape" => {
                cx.stop_propagation();
                self.cancel(window, cx);
                window.blur();
                cx.emit(ToolbarEvent::NameEditingFinished);
            }
            _ => {}
        }
    }

    /// Holds the window still only while a press on the name would be the user
    /// reaching for the text in it. The bar is the window's drag handle and the
    /// platform acts on that before this application sees the press, so the
    /// window has to be still *before* the press — which means following the
    /// pointer rather than waiting for it to be pressed.
    fn apply_window_movable(&mut self, _window: &mut Window) {
        let movable = !(self.editing && self.name_hovered);
        if movable != self.window_movable {
            self.window_movable = movable;
            #[cfg(target_os = "macos")]
            platform::macos::set_window_movable(_window, movable);
        }
    }

    /// Whether the press that is finishing carried the window with it. Dragging
    /// the window is the platform's own doing, and it leaves the press looking
    /// like an ordinary click on whatever it started over — so what the press
    /// did to the window is what tells the two apart.
    fn window_moved(&self, window: &Window) -> bool {
        self.press
            .is_some_and(|press| moved(press.window_origin, window.bounds().origin))
    }

    /// Whether the press that is finishing began over the name. A release there
    /// after a press somewhere else is the end of that other press.
    fn pressed_name(&self) -> bool {
        self.press.is_some_and(|press| press.on_name)
    }

    pub fn toggle_pin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pinned = !self.pinned;
        platform::set_always_on_top(window, self.pinned);
        cx.notify();
    }
}

impl Render for Toolbar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Avoid sending the same title to the native window on every render.
        let window_title = if self.title.is_empty() {
            SharedString::new_static("Siffra")
        } else {
            self.title.clone()
        };
        if self.window_title.as_ref() != Some(&window_title) {
            window.set_window_title(&window_title);
            self.window_title = Some(window_title);
        }

        // The centered title must clear both the traffic lights and the buttons.
        let window_width = f32::from(window.viewport_size().width);
        let title_max = theme::TITLE_MAX_WIDTH
            .min(window_width - theme::TITLE_SIDE_RESERVE)
            .max(theme::TITLE_MIN_WIDTH);
        self.name
            .update(cx, |name, _| name.set_max_width(Some(px(title_max))));

        div()
            .h(px(theme::NAV_HEIGHT))
            .flex_shrink_0()
            .relative()
            .bg(theme::toolbar_bg())
            .font_family(UI_FONT_FAMILY)
            // The custom title bar remains the window's drag target.
            .window_control_area(WindowControlArea::Drag)
            // Noted before anything in the bar acts on the press, so that every
            // one of them can tell a click from the start of a window drag.
            .capture_any_mouse_down(cx.listener(|toolbar, _: &MouseDownEvent, window, _| {
                toolbar.press = Some(Press {
                    window_origin: window.bounds().origin,
                    on_name: false,
                });
            }))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .child(
                div()
                    // Identified so its hover state survives between frames.
                    .id("toolbar-title")
                    // Handle these keys at the toolbar level so they can commit
                    // or cancel the document name edit.
                    .on_key_down(cx.listener(Self::on_name_key_down))
                    // The window is only held still while the pointer is over
                    // the name, so that the rest of the bar goes on dragging it
                    // — including the press that ends the rename.
                    .on_hover(cx.listener(|toolbar, hovered: &bool, window, _| {
                        toolbar.name_hovered = *hovered;
                        toolbar.apply_window_movable(window);
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|toolbar, _: &MouseDownEvent, _, _| {
                            if let Some(press) = toolbar.press.as_mut() {
                                press.on_name = true;
                            }
                        }),
                    )
                    // An idle field starts editing instead of placing a caret,
                    // and only once the press it started with is over: until
                    // then the press may yet turn out to be a window drag.
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|toolbar, _: &MouseUpEvent, window, cx| {
                            if toolbar.editing
                                || !toolbar.pressed_name()
                                || toolbar.window_moved(window)
                            {
                                return;
                            }
                            cx.stop_propagation();
                            toolbar.start_editing(window, cx);
                        }),
                    )
                    // Result buttons do not necessarily take keyboard focus,
                    // so focus loss alone is not enough to finish a rename.
                    .when(self.editing, |title| {
                        title.on_mouse_down_out(cx.listener(
                            |toolbar, _: &MouseDownEvent, window, cx| {
                                toolbar.commit(window, cx);
                                window.blur();
                            },
                        ))
                    })
                    .child(self.name.clone()),
            )
            .child(self.render_menu(cx))
    }
}

impl Toolbar {
    fn render_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .right_0()
            .h(px(theme::NAV_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .p(px(theme::TOOLBAR_MENU_PADDING))
            .child(
                icon_button("toolbar-new", icon::PLUS)
                    .tooltip(tooltip(format!("New calculation ({MOD_KEY}N)")))
                    .on_click(cx.listener(|toolbar, _: &ClickEvent, window, cx| {
                        if toolbar.window_moved(window) {
                            return;
                        }
                        cx.emit(ToolbarEvent::NewDocument);
                    })),
            )
            .child(
                icon_button("toolbar-search", icon::SEARCH)
                    .tooltip(tooltip(format!("Search ({MOD_KEY}K)")))
                    .on_click(cx.listener(|toolbar, _: &ClickEvent, window, cx| {
                        if toolbar.window_moved(window) {
                            return;
                        }
                        cx.emit(ToolbarEvent::OpenSearch);
                    })),
            )
            .child(
                icon_button("toolbar-help", icon::HELP)
                    .tooltip(tooltip(format!("Help ({MOD_KEY}?)")))
                    .when(self.help_open, |button| {
                        button.bg(theme::toolbar_hover_bg())
                    })
                    .on_click(cx.listener(|toolbar, _: &ClickEvent, window, cx| {
                        if toolbar.window_moved(window) {
                            return;
                        }
                        cx.emit(ToolbarEvent::ToggleHelp);
                    })),
            )
            .child(
                pin_toggle(self.pinned)
                    .tooltip(tooltip(format!("Always on top ({MOD_KEY}P)")))
                    .on_click(cx.listener(|toolbar, _: &ClickEvent, window, cx| {
                        if toolbar.window_moved(window) {
                            return;
                        }
                        toolbar.toggle_pin(window, cx);
                    })),
            )
    }
}

/// Uses the toolbar font and a full line-height caret.
fn name_field_metrics() -> FieldMetrics {
    FieldMetrics {
        padding_x: px(theme::TITLE_PADDING_X),
        padding_y: px(theme::TITLE_PADDING_Y),
        caret_height: px(theme::TITLE_LINE_HEIGHT),
        selection: theme::title_selection(),
        selection_text: Some(theme::title_selection_text()),
        placeholder: theme::title_placeholder(),
        centered: true,
    }
}

/// Editing wins over hover.
fn name_field_chrome(field: Stateful<Div>, state: FieldState) -> Stateful<Div> {
    field
        .flex_none()
        .min_w(px(theme::TITLE_MIN_WIDTH))
        .truncate()
        .rounded(px(theme::TITLE_RADIUS))
        .text_size(px(theme::TITLE_FONT))
        .line_height(px(theme::TITLE_LINE_HEIGHT))
        .font_weight(FontWeight::BOLD)
        .text_color(theme::title_text())
        .map(|field| {
            if state.focused {
                field
                    .bg(theme::title_editing_bg())
                    .text_color(theme::title_editing_text())
                    .cursor(CursorStyle::IBeam)
            } else {
                field.cursor(CursorStyle::Arrow).hover(|style| {
                    style
                        .bg(theme::toolbar_hover_bg())
                        .text_color(theme::title_hover_text())
                })
            }
        })
}

fn tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView {
    let text = text.into();
    move |_, cx| {
        let text = text.clone();
        cx.new(|_| TextTooltip::new(text)).into()
    }
}

/// Whether the window travelled far enough between `from` and `to` for the press
/// that spanned them to have been a drag.
fn moved(from: Point<Pixels>, to: Point<Pixels>) -> bool {
    (to - from).magnitude() > DRAG_SLOP
}

/// Builds a toolbar icon button whose glyph follows the button's hover colour.
///
/// GPUI tints SVGs from the element's own colour rather than inherited colour,
/// so the group keeps the button and glyph in sync.
fn icon_button(id: &'static str, path: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .group(id)
        .flex_none()
        .size(px(theme::ICON_BUTTON_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::ICON_BUTTON_RADIUS))
        .cursor(CursorStyle::Arrow)
        .hover(|style| style.bg(theme::toolbar_hover_bg()))
        .child(
            svg()
                .path(path)
                .size(px(theme::ICON_SIZE))
                .flex_none()
                .text_color(theme::toolbar_icon())
                .group_hover(id, |style| style.text_color(theme::toolbar_icon_hover())),
        )
}

/// Builds the pin button; its pinned state remains visible while hovered.
fn pin_toggle(pinned: bool) -> Stateful<Div> {
    const GROUP: &str = "toolbar-pin";

    div()
        .id(GROUP)
        .group(GROUP)
        .flex_none()
        .p(px(theme::TOGGLE_PADDING))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(theme::TOGGLE_RADIUS))
        .when(pinned, |button| button.bg(theme::toggle_on_bg()))
        .cursor(CursorStyle::Arrow)
        .when(!pinned, |button| {
            button.hover(|style| style.bg(theme::toolbar_hover_bg()))
        })
        .active(|style| style.bg(theme::toggle_active_bg()))
        .child(
            svg()
                .path(icon::PIN)
                .size(px(theme::TOGGLE_ICON_SIZE))
                .flex_none()
                .text_color(if pinned {
                    theme::toggle_on_icon()
                } else {
                    theme::toggle_icon()
                }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::point;

    /// A press that took the window with it was a drag of the window, whatever
    /// it happened to start over.
    #[test]
    fn a_press_that_moves_the_window_is_a_drag() {
        let origin = point(px(100.0), px(100.0));
        assert!(!moved(origin, origin));
        assert!(!moved(origin, point(px(101.0), px(101.0))));
        assert!(moved(origin, point(px(100.0), px(104.0))));
    }
}
