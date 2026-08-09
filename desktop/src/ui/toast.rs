use crate::ui::easing::cubic_out;
use crate::ui::theme;
use gpui::{BoxShadow, Div, FontWeight, ParentElement, SharedString, Styled, div, point, px};
use std::time::Instant;

#[derive(Clone, Copy)]
struct Phase {
    opacity: f32,
    rise: f32,
}

#[derive(Default)]
pub struct Toast {
    message: Option<SharedString>,
    raised_at: Option<Instant>,
}

impl Toast {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn show(&mut self, message: impl Into<SharedString>) {
        self.message = Some(message.into());
        self.raised_at = Some(Instant::now());
    }

    fn phase(&self) -> Option<Phase> {
        let elapsed = self.raised_at?.elapsed().as_secs_f32();
        let fade = theme::TOAST_FADE;
        let hold = theme::TOAST_HOLD;

        let shown = if elapsed < fade {
            cubic_out(elapsed / fade)
        } else if elapsed < fade + hold {
            1.0
        } else if elapsed < fade + hold + fade {
            cubic_out(1.0 - (elapsed - fade - hold) / fade)
        } else {
            return None;
        };

        Some(Phase {
            opacity: shown,
            rise: (1.0 - shown) * theme::TOAST_RISE,
        })
    }

    pub fn animating(&self) -> bool {
        self.phase().is_some()
    }

    pub fn render(&self) -> Option<Div> {
        let phase = self.phase()?;
        let message = self.message.clone()?;

        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(theme::TOAST_BOTTOM - phase.rise))
                .flex()
                .justify_center()
                .opacity(phase.opacity)
                .child(
                    div()
                        .px(px(theme::TOAST_PADDING_X))
                        .py(px(theme::TOAST_PADDING_Y))
                        .rounded(px(theme::TOAST_RADIUS))
                        .bg(theme::toast_bg())
                        .border_1()
                        .border_color(theme::toast_border())
                        .shadow(vec![BoxShadow {
                            color: theme::hsla(theme::toast_shadow()),
                            offset: point(px(0.0), px(4.0)),
                            blur_radius: px(14.0),
                            spread_radius: px(0.0),
                        }])
                        .text_color(theme::toast_text())
                        .text_size(px(theme::TOAST_FONT))
                        .line_height(px(theme::ROOT_LINE_HEIGHT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(message),
                ),
        )
    }
}
