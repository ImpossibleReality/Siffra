use crate::ui::theme;
use gpui::{
    CursorStyle, Div, ElementId, FontWeight, InteractiveElement, ParentElement, SharedString,
    Stateful, Styled, div, px,
};

fn base(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px(px(theme::BUTTON_PADDING_X))
        .py(px(theme::BUTTON_PADDING_Y))
        .rounded(px(theme::BUTTON_RADIUS))
        .text_size(px(theme::FONT_SMALL))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .cursor(CursorStyle::Arrow)
        .child(label.into())
}

/// A quiet button for the action that leaves things as they were.
pub fn secondary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    base(id, label)
        .text_color(theme::button_text())
        .hover(|style| {
            style
                .bg(theme::button_hover_bg())
                .text_color(theme::button_hover_text())
        })
}

/// The filled button for a dialog's default action.
pub fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    base(id, label)
        .bg(theme::primary_button_bg())
        .text_color(theme::primary_button_text())
        .hover(|style| style.bg(theme::primary_button_hover_bg()))
}
