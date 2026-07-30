//! The bar across the top of the window: what the calculation is called, and
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

use crate::assets::icon;
use crate::platform;
use crate::theme;
use crate::ui::UI_FONT;
use crate::ui::single_line_input::{FieldMetrics, FieldState, SingleLineInput, ValueChanged};
use crate::ui::tooltip::TextTooltip;
use gpui::{
    AnyView, App, AppContext, ClickEvent, Context, CursorStyle, Div, Entity, EventEmitter,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement, Render, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Subscription, Window, WindowControlArea, div, prelude::FluentBuilder, px, svg,
};

/// The toolbar uses the interface font rather than the editor's monospace font.
const UI_FONT_FAMILY: &str = UI_FONT;

#[cfg(target_os = "macos")]
const MOD_KEY: &str = "⌘";
#[cfg(not(target_os = "macos"))]
const MOD_KEY: &str = "Ctrl ";

/// What the bar asks the workspace for. The name is reported as it was typed;
/// what an empty one falls back to is the store's business.
pub enum ToolbarEvent {
    NewDocument,
    OpenSearch,
    ToggleHelp,
    Rename(String),
    /// The name field is finished with the keyboard, which is the workspace's cue
    /// to hand focus back to the calculation.
    NameEditingFinished,
}

pub struct Toolbar {
    /// What the open calculation is called. Empty until it has been named, when
    /// the field shows its placeholder instead.
    title: SharedString,
    /// The name field. It holds what the user is typing; while the field is idle
    /// it is kept in step with `title`, so a rename from anywhere else can never
    /// be clobbered by a stale draft.
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
    _subscriptions: Vec<Subscription>,
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
        self.blur_subscription = Some(cx.on_blur(&handle, window, |toolbar, _, cx| {
            toolbar.commit(cx);
        }));
        cx.notify();
    }

    /// Hands the name back to the workspace, unless nothing about it changed.
    fn commit(&mut self, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        self.blur_subscription = None;
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
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        self.editing = false;
        self.blur_subscription = None;
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
                self.commit(cx);
                window.blur();
                cx.emit(ToolbarEvent::NameEditingFinished);
            }
            "escape" => {
                cx.stop_propagation();
                self.cancel(cx);
                window.blur();
                cx.emit(ToolbarEvent::NameEditingFinished);
            }
            _ => {}
        }
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
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .child(
                div()
                    // Handle these keys at the toolbar level so they can commit
                    // or cancel the document name edit.
                    .on_key_down(cx.listener(Self::on_name_key_down))
                    // An idle field starts editing instead of placing a caret.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|toolbar, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            toolbar.start_editing(window, cx);
                        }),
                    )
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
                    .tooltip(tooltip("New calculation"))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                        cx.emit(ToolbarEvent::NewDocument);
                    })),
            )
            .child(
                icon_button("toolbar-search", icon::SEARCH)
                    .tooltip(tooltip(format!("Search calculations ({MOD_KEY}K)")))
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                        cx.emit(ToolbarEvent::OpenSearch);
                    })),
            )
            .child(
                icon_button("toolbar-help", icon::HELP)
                    .tooltip(tooltip("Help"))
                    .when(self.help_open, |button| {
                        button.bg(theme::toolbar_hover_bg())
                    })
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                        cx.emit(ToolbarEvent::ToggleHelp);
                    })),
            )
            .child(
                pin_toggle(self.pinned)
                    .tooltip(tooltip("Always on top"))
                    .on_click(cx.listener(|toolbar, _: &ClickEvent, window, cx| {
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
