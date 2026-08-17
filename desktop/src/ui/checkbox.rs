use crate::ui::theme;
use gpui::{
    CursorStyle, Div, ElementId, InteractiveElement, ParentElement, SharedString, Stateful, Styled,
    div, prelude::FluentBuilder, px,
};

/// A labelled checkbox
pub fn checkbox(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
) -> Stateful<Div> {
    const GROUP: &str = "checkbox";

    div()
        .id(id)
        .group(GROUP)
        .flex()
        .items_center()
        .gap(px(theme::CHECKBOX_GAP))
        .text_size(px(theme::FONT_SMALL))
        .text_color(theme::checkbox_label())
        .cursor(CursorStyle::Arrow)
        .hover(|style| style.text_color(theme::button_hover_text()))
        .child(
            div()
                .flex_none()
                .size(px(theme::CHECKBOX_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(theme::CHECKBOX_RADIUS))
                .text_size(px(theme::FONT_TINY))
                .map(|box_| {
                    if checked {
                        box_.bg(theme::checkbox_on_bg())
                            .text_color(theme::checkbox_on_mark())
                            .child("✓")
                    } else {
                        box_.border_1()
                            .border_color(theme::checkbox_border())
                            .group_hover(GROUP, |style| {
                                style.border_color(theme::checkbox_hover_border())
                            })
                    }
                }),
        )
        .child(label.into())
}
