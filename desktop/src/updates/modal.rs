//! The dialog that presents a release

use crate::ui::assets::image;
use crate::ui::button::{primary_button, secondary_button};
use crate::ui::checkbox::checkbox;
use crate::ui::markdown::{Block, MarkdownStyle, markdown, parse};
use crate::ui::modal::{ModalSize, overlay, panel};
use crate::ui::theme;
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, img, px,
};

pub enum UpdateModalEvent {
    Dismiss { never_again: bool },
    Install,
}

pub struct UpdateModal {
    focus_handle: FocusHandle,
    mono_family: SharedString,
    current_version: SharedString,
    next_version: SharedString,
    notes: Vec<Block>,
    never_again: bool,
    installing: bool,
}

impl EventEmitter<UpdateModalEvent> for UpdateModal {}

impl UpdateModal {
    pub fn new(mono_family: SharedString, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            mono_family,
            current_version: SharedString::default(),
            next_version: SharedString::default(),
            notes: Vec::new(),
            never_again: false,
            installing: false,
        }
    }

    pub fn open(
        &mut self,
        current_version: SharedString,
        next_version: SharedString,
        notes: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.current_version = current_version;
        self.next_version = next_version;
        self.notes = parse(notes);
        self.never_again = false;
        self.installing = false;
        self.focus_handle.focus(window);
        cx.notify();
    }

    pub fn set_installing(&mut self, installing: bool, cx: &mut Context<Self>) {
        self.installing = installing;
        cx.notify();
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(UpdateModalEvent::Dismiss {
            never_again: self.never_again,
        });
    }

    fn install(&mut self, cx: &mut Context<Self>) {
        if !self.installing {
            cx.emit(UpdateModalEvent::Install);
        }
    }

    fn toggle_never_again(&mut self, cx: &mut Context<Self>) {
        self.never_again = !self.never_again;
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" {
            cx.stop_propagation();
            self.dismiss(cx);
        }
    }

    fn render_summary(&self) -> impl IntoElement {
        div()
            .flex_none()
            .w(px(theme::UPDATE_SIDEBAR_WIDTH))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(0.5 * theme::REM))
            .p(px(theme::REM))
            .bg(theme::update_sidebar_bg())
            .border_r_1()
            .border_color(theme::update_divider())
            .child(
                img(image::APP_ICON)
                    .size(px(theme::UPDATE_APP_ICON_SIZE))
                    .flex_none(),
            )
            .child(
                div()
                    .text_size(px(1.15 * theme::REM))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::update_app_name())
                    .child("Siffra"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(0.35 * theme::REM))
                    .text_size(px(theme::FONT_SMALL))
                    .text_color(theme::update_version())
                    .child(self.current_version.clone())
                    .child("→")
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::update_version_next())
                            .child(self.next_version.clone()),
                    ),
            )
    }

    fn render_notes(&self) -> impl IntoElement {
        div()
            .id("update-notes")
            .flex_1()
            .min_w_0()
            .p(px(1.2 * theme::REM))
            .overflow_y_scroll()
            .child(markdown(
                "update-notes-markdown",
                &self.notes,
                &MarkdownStyle::new(px(0.85 * theme::REM), self.mono_family.clone()),
            ))
    }

    fn render_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(theme::REM))
            .px(px(0.9 * theme::REM))
            .py(px(0.7 * theme::REM))
            .border_t_1()
            .border_color(theme::update_divider())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(0.6 * theme::REM))
                    .child(
                        secondary_button("update-dismiss", "Dismiss").on_click(cx.listener(
                            |modal, _, _, cx| {
                                modal.dismiss(cx);
                            },
                        )),
                    )
                    .child(
                        checkbox("update-never-again", "Ignore this update", self.never_again)
                            .on_click(cx.listener(|modal, _, _, cx| {
                                modal.toggle_never_again(cx);
                            })),
                    ),
            )
            .child(
                primary_button(
                    "update-install",
                    if self.installing {
                        "Installing…"
                    } else {
                        "Install Update"
                    },
                )
                .on_click(cx.listener(|modal, _, _, cx| {
                    modal.install(cx);
                })),
            )
    }
}

impl Render for UpdateModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        overlay("update-overlay")
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|modal, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    modal.dismiss(cx);
                }),
            )
            .child(
                panel(
                    "update-panel",
                    ModalSize::new(
                        px(theme::UPDATE_PANEL_WIDTH),
                        px(theme::UPDATE_PANEL_HEIGHT),
                    ),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _: &MouseDownEvent, _, cx| cx.stop_propagation()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .flex_row()
                        .child(self.render_summary())
                        .child(self.render_notes()),
                )
                .child(self.render_actions(cx)),
            )
    }
}

impl Focusable for UpdateModal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
