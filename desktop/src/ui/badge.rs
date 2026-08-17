use crate::ui::theme;
use gpui::{Div, Rgba, Styled, div, px};

/// The notification dot that sits over the top right corner of a control.
///
/// The ring is drawn in the colour the dot sits on so the dot keeps its shape
/// where it overlaps the control's own edge.
pub fn dot(ring: Rgba) -> Div {
    div()
        .absolute()
        .top(px(theme::BADGE_INSET))
        .right(px(theme::BADGE_INSET))
        .size(px(theme::BADGE_SIZE))
        .rounded_full()
        .bg(theme::badge())
        .border_1()
        .border_color(ring)
}

/// A dot that sits inline beside a label rather than over a control.
pub fn inline_dot() -> Div {
    div()
        .flex_none()
        .size(px(theme::BADGE_SIZE))
        .rounded_full()
        .bg(theme::badge())
}
