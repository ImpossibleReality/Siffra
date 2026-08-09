//! A panel that hangs off whatever opened it.
//!
//! Only the chrome lives here — the frame, the padding and the type it sets its
//! contents in. What goes inside is the caller's, and so is deciding when it is
//! open; a popover is a shape, not a thing that owns state.
//!
//! Placing one is the caller's too, because the thing it hangs off is generally
//! drawn somewhere this element cannot see.

use crate::ui::theme;
use gpui::{
    BoxShadow, CursorStyle, Div, ElementId, FontWeight, InteractiveElement, ParentElement, Pixels,
    SharedString, Stateful, Styled, div, point, px,
};

/// The bounded, padded frame for popover contents.
///
/// It shrinks to its contents between a floor and a ceiling, so a short message
/// still reads as a panel and a long one still stops before it takes the window.
pub fn popover() -> Div {
    div()
        .min_w(px(theme::POPOVER_MIN_WIDTH))
        .max_w(px(theme::POPOVER_MAX_WIDTH))
        .p(px(theme::POPOVER_PADDING))
        .bg(theme::popover_bg())
        .border_1()
        .border_color(theme::popover_border())
        .rounded(px(theme::POPOVER_RADIUS))
        .shadow(vec![BoxShadow {
            color: theme::hsla(theme::popover_shadow()),
            offset: point(px(0.0), px(6.0)),
            blur_radius: px(20.0),
            spread_radius: px(0.0),
        }])
        .text_size(px(theme::REM))
        .line_height(px(theme::ROOT_LINE_HEIGHT))
        .font_weight(FontWeight::MEDIUM)
        // Nothing in a popover is a text field, and a caret over one of them
        // would suggest it were.
        .cursor(CursorStyle::Arrow)
}

/// Places a popover below its anchor with their right edges aligned.
///
/// Both distances are from the same edges of whichever ancestor the popover is
/// hung on, so the caller works out where the anchor's bottom right corner
/// landed and this puts the panel below it.
pub fn below_right(bottom: Pixels, right: Pixels) -> Div {
    popover()
        .absolute()
        .top(bottom + px(theme::POPOVER_GAP))
        .right(right)
}

/// A full-width action button inside a popover.
pub fn popover_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .px(px(theme::POPOVER_BUTTON_PADDING_X))
        .py(px(theme::POPOVER_BUTTON_PADDING_Y))
        .rounded(px(theme::POPOVER_BUTTON_RADIUS))
        .bg(theme::popover_button_bg())
        .text_color(theme::popover_button_text())
        .text_size(px(theme::FONT_SMALL))
        .font_weight(FontWeight::BOLD)
        .text_center()
        .cursor(CursorStyle::Arrow)
        .hover(|style| {
            style
                .bg(theme::popover_button_hover_bg())
                .text_color(theme::popover_button_hover_text())
        })
        .child(label.into())
}
