use crate::ui::popover::popover;
use crate::ui::theme;
use gpui::{
    CursorStyle, Div, ElementId, FontWeight, InteractiveElement, ParentElement, SharedString,
    Stateful, Styled, div, px,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Check {
    None,
    Off,
    On,
}

/// The frame a set of [`menu_item`]s sits in.
pub fn menu() -> Div {
    popover()
        .min_w(px(theme::MENU_MIN_WIDTH))
        .p(px(theme::MENU_PADDING))
        .flex()
        .flex_col()
        .text_size(px(theme::MENU_FONT))
        .line_height(px(theme::MENU_LINE_HEIGHT))
        .font_weight(FontWeight::NORMAL)
}

/// A disabled row for reporting status
pub fn menu_status(label: impl Into<SharedString>) -> Div {
    row()
        .text_color(theme::menu_status_text())
        .child(label.into())
}

fn row() -> Div {
    div()
        .flex()
        .items_center()
        .px(px(theme::MENU_ITEM_PADDING_X))
        .py(px(theme::MENU_ITEM_PADDING_Y))
        .rounded(px(theme::MENU_ITEM_RADIUS))
        .whitespace_nowrap()
}

pub fn menu_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    check: Check,
) -> Stateful<Div> {
    row()
        .id(id)
        .text_color(theme::menu_item_text())
        .cursor(CursorStyle::Arrow)
        .hover(|style| {
            style
                .bg(theme::menu_item_hover_bg())
                .text_color(theme::menu_item_hover_text())
        })
        .child(label.into())
        .children((check != Check::None).then(|| {
            div()
                .flex_none()
                .ml_auto()
                .pl(px(theme::MENU_ITEM_GAP))
                .text_size(px(theme::MENU_FONT))
                .text_color(theme::menu_check())
                .child(if check == Check::On { "✓" } else { "" })
        }))
}
