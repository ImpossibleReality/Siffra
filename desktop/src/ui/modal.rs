//! Shared modal chrome.
//!
//! The caller owns visibility, dismissal, keyboard handling, and contents.
//! Keeping those decisions outside the frame lets help, file search, and save
//! palettes use the same shape without coupling their state machines.

use crate::ui::UI_FONT;
use crate::ui::theme;
use gpui::{
    BoxShadow, CursorStyle, Div, ElementId, FontWeight, InteractiveElement, ParentElement, Pixels,
    Stateful, Styled, div, point, px, rgb, rgba,
};

#[derive(Clone, Copy)]
pub struct ModalSize {
    pub max_width: Pixels,
    pub max_height: Pixels,
}

impl ModalSize {
    pub fn new(max_width: Pixels, max_height: Pixels) -> Self {
        Self {
            max_width,
            max_height,
        }
    }
}

/// The dimmed, window-filling layer behind a modal panel.
pub fn overlay(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .pt(px(3.0 * theme::REM))
        .px(px(theme::REM))
        .pb(px(theme::REM))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x00000080))
        .font_family(UI_FONT)
        .font_weight(FontWeight::NORMAL)
        .cursor(CursorStyle::Arrow)
}

/// The bordered and shadowed panel that contains a modal feature. It takes all
/// the height it is allowed, which is what keeps a long list of contents from
/// changing the frame's size as it is filtered.
pub fn panel(id: impl Into<ElementId>, size: ModalSize) -> Stateful<Div> {
    panel_chrome(id, size).h_full()
}

/// [`panel`] for a modal that is only as tall as what it holds, up to its
/// ceiling — a palette that starts as one row and grows.
pub fn fitted_panel(id: impl Into<ElementId>, size: ModalSize) -> Stateful<Div> {
    panel_chrome(id, size)
}

fn panel_chrome(id: impl Into<ElementId>, size: ModalSize) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .max_w(size.max_width)
        .max_h(size.max_height)
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(0x1e2026))
        .border_1()
        .border_color(rgb(0x3a3f49))
        .rounded(px(0.6 * theme::REM))
        .shadow(vec![BoxShadow {
            color: rgba(0x00000080).into(),
            offset: point(px(0.0), px(16.0)),
            blur_radius: px(50.0),
            spread_radius: px(0.0),
        }])
}

/// A modal title/action row.
pub fn header() -> Div {
    div()
        .px(px(0.9 * theme::REM))
        .pt(px(0.8 * theme::REM))
        .pb(px(0.6 * theme::REM))
        .flex()
        .items_center()
        .justify_between()
}

/// The standard compact close affordance. The caller attaches its click action.
pub fn close_button(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(0.45 * theme::REM))
        .py(px(0.25 * theme::REM))
        .rounded(px(0.35 * theme::REM))
        .text_size(px(0.85 * theme::REM))
        .line_height(px(theme::REM))
        .text_color(rgb(0x8a9196))
        .cursor(CursorStyle::Arrow)
        .hover(|style| style.bg(rgba(0xffffff0a)).text_color(rgb(0xcdd4d7)))
        .child("✕")
}

/// The inset, flexible body used by scrolling modal contents.
pub fn body(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .min_h_0()
        .px(px(0.9 * theme::REM))
        .pb(px(0.9 * theme::REM))
}
