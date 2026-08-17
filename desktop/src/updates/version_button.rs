//! Version button shown in help menu

use super::UpdateStatus;
use crate::ui::badge;
use crate::ui::menu::{Check, menu, menu_item, menu_status};
use crate::ui::theme;
use gpui::{
    Context, CursorStyle, EventEmitter, FontWeight, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Window, deferred, div, prelude::FluentBuilder, px,
};

/// Where a check has got to, for the menu's status line.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Idle,
    Checking,
    Failed,
}

pub enum VersionButtonEvent {
    CheckForUpdates,
    ShowUpdate,
    SetAutoUpdate(bool),
}

pub struct VersionButton {
    status: UpdateStatus,
    menu_open: bool,
    check: CheckState,
}

impl EventEmitter<VersionButtonEvent> for VersionButton {}

impl VersionButton {
    pub fn new(status: UpdateStatus) -> Self {
        Self {
            status,
            menu_open: false,
            check: CheckState::Idle,
        }
    }

    pub fn set_status(&mut self, status: UpdateStatus, cx: &mut Context<Self>) {
        if self.status != status {
            self.status = status;
            cx.notify();
        }
    }

    pub fn set_check_state(&mut self, check: CheckState, cx: &mut Context<Self>) {
        if self.check != check {
            self.check = check;
            cx.notify();
        }
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu_open {
            self.menu_open = false;
            cx.notify();
        }
    }

    fn press(&mut self, cx: &mut Context<Self>) {
        if self.status.announced {
            self.menu_open = false;
            cx.emit(VersionButtonEvent::ShowUpdate);
        } else {
            self.menu_open = !self.menu_open;
        }
        cx.notify();
    }

    fn choose(&mut self, event: VersionButtonEvent, cx: &mut Context<Self>) {
        self.menu_open = false;
        cx.emit(event);
        cx.notify();
    }

    /// Acts without dismissing the menu, so its status line can report what
    /// happened where the pointer already is.
    fn choose_in_place(&mut self, event: VersionButtonEvent, cx: &mut Context<Self>) {
        cx.emit(event);
    }

    fn status_label(&self) -> &'static str {
        match self.check {
            CheckState::Checking => "Checking for Updates…",
            CheckState::Failed => "Check Failed",
            CheckState::Idle if self.status.available() => "Update Available",
            CheckState::Idle => "Latest Version",
        }
    }

    fn label(&self) -> SharedString {
        if self.status.announced {
            SharedString::new_static("Update Available")
        } else {
            SharedString::from(format!("v{}", self.status.current_version))
        }
    }
}

impl Render for VersionButton {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let announced = self.status.announced;
        div()
            .relative()
            .flex_none()
            .child(
                div()
                    .id("help-version")
                    .flex()
                    .items_center()
                    .gap(px(0.4 * theme::REM))
                    .px(px(theme::VERSION_BUTTON_PADDING_X))
                    .py(px(theme::VERSION_BUTTON_PADDING_Y))
                    .rounded(px(theme::VERSION_BUTTON_RADIUS))
                    .text_size(px(theme::FONT_TINY))
                    .font_weight(FontWeight::SEMIBOLD)
                    .whitespace_nowrap()
                    .text_color(if announced {
                        theme::version_button_alert_text()
                    } else {
                        theme::version_button_text()
                    })
                    .cursor(CursorStyle::Arrow)
                    .hover(|style| {
                        style
                            .bg(theme::version_button_hover_bg())
                            .text_color(theme::version_button_hover_text())
                    })
                    .when(self.menu_open, |button| {
                        button.bg(theme::version_button_hover_bg())
                    })
                    .child(self.label())
                    .children(announced.then(badge::inline_dot))
                    // Kept from the panel's own handler so opening the menu is
                    // not undone by the press that opened it.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _: &MouseDownEvent, _, cx| cx.stop_propagation()),
                    )
                    .on_click(cx.listener(|button, _, _, cx| button.press(cx))),
            )
            // Deferred so the menu paints over the dialog's body rather than
            // under it, the header being drawn first.
            .children(self.menu_open.then(|| deferred(self.render_menu(cx))))
    }
}

impl VersionButton {
    fn render_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let auto_update = self.status.auto_update;
        // An available release the user has ignored is still reachable, so the
        // action offers it rather than a check that would only find it again.
        let update_waiting = self.status.available();

        menu()
            .absolute()
            .top_full()
            .right_0()
            .mt(px(theme::POPOVER_GAP))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| cx.stop_propagation()),
            )
            .child(menu_status(self.status_label()))
            .child(if update_waiting {
                menu_item("version-menu-install", "Install Update", Check::None).on_click(
                    cx.listener(|button, _, _, cx| {
                        button.choose(VersionButtonEvent::ShowUpdate, cx);
                    }),
                )
            } else {
                menu_item("version-menu-check", "Check for Updates", Check::None).on_click(
                    cx.listener(|button, _, _, cx| {
                        button.choose_in_place(VersionButtonEvent::CheckForUpdates, cx);
                    }),
                )
            })
            .child(
                menu_item(
                    "version-menu-auto",
                    "Auto Update",
                    if auto_update { Check::On } else { Check::Off },
                )
                .on_click(cx.listener(move |button, _, _, cx| {
                    button.choose(VersionButtonEvent::SetAutoUpdate(!auto_update), cx);
                })),
            )
    }
}
