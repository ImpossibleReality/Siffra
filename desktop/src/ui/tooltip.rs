//! The small text tooltip shared by compact controls.

use crate::ui::UI_FONT;
use crate::ui::theme;
use gpui::{
    Context, FontWeight, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px,
    rgb,
};

pub struct TextTooltip {
    text: SharedString,
}

impl TextTooltip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl Render for TextTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(0.4 * theme::REM))
            .py(px(0.2 * theme::REM))
            .bg(rgb(0x3f444d))
            .border_1()
            .border_color(rgb(0x4b515c))
            .rounded(px(0.3 * theme::REM))
            .whitespace_nowrap()
            .font_family(UI_FONT)
            .text_size(px(0.8 * theme::REM))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xcdd4d7))
            .child(self.text.clone())
    }
}
