use self::format::{NumberDisplay, format_number_for_characters, format_repeating_decimal};
use super::layout::{Fonts, Metrics, row_rects};
use crate::calculation::LineOutput;

pub mod format;
use crate::ui::theme;
use gpui::{
    App, Bounds, Element, ElementId, Font, GlobalElementId, Hsla, InspectorElementId, IntoElement,
    LayoutId, Pixels, Rgba, ShapedLine, SharedString, Style, TextRun, Window, WrappedLine, fill,
    point, px, size,
};
use std::ops::Range;

const LINE_HEIGHT: f32 = 24.0;

const VINCULUM_THICKNESS: f32 = 1.0;

struct Text {
    text: SharedString,
    repetend: Option<Range<usize>>,
    characters: usize,
}

fn text_for(output: &LineOutput, budget: usize) -> Text {
    match output {
        // Rounding is only meaningful for pure numeric values
        LineOutput::Value {
            string,
            numeric: false,
            ..
        } => Text {
            text: string.clone().into(),
            repetend: None,
            characters: string.chars().count(),
        },
        LineOutput::Value {
            string, repeating, ..
        } => {
            let display = format_number_for_characters(string, budget, *repeating);
            let characters = display.len();
            let NumberDisplay {
                before_repeat,
                repeating: repetend,
                after_repeat,
            } = display;

            let start = before_repeat.len();
            let end = start + repetend.len();
            Text {
                text: format!("{before_repeat}{repetend}{after_repeat}").into(),
                repetend: (!repetend.is_empty()).then_some(start..end),
                characters,
            }
        }
        LineOutput::Error { message, .. } => Text {
            text: message.clone().into(),
            repetend: None,
            characters: message.chars().count(),
        },
    }
}

pub fn width(output: &LineOutput, budget: usize, metrics: Metrics) -> Pixels {
    metrics.bold_char_width * text_for(output, budget).characters as f32
        + px(2.0 * theme::OUTPUT_PADDING_X)
}

fn scaled_about_center(bounds: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    if scale == 1.0 {
        return bounds;
    }

    let center = bounds.center();
    let size = size(bounds.size.width * scale, bounds.size.height * scale);
    Bounds::new(
        point(center.x - size.width / 2.0, center.y - size.height / 2.0),
        size,
    )
}

#[derive(Copy, Clone, PartialEq, Eq)]
/// Result interaction state; variant order defines styling precedence.
pub enum Interaction {
    Idle,
    Hovered,
    Pressed,
    Open,
}

impl Interaction {
    fn colors(self, is_error: bool) -> (Rgba, Option<Rgba>) {
        match (self, is_error) {
            (Interaction::Idle, false) => (theme::result_value(), None),
            (Interaction::Idle, true) => (theme::result_error(), None),
            (Interaction::Hovered, false) => (
                theme::result_hover_text(),
                Some(theme::result_value_hover_bg()),
            ),
            (Interaction::Hovered, true) => (
                theme::result_error_hover_text(),
                Some(theme::result_error_hover_bg()),
            ),
            (Interaction::Pressed, false) => (
                theme::result_hover_text(),
                Some(theme::result_value_active_bg()),
            ),
            (Interaction::Pressed, true) => (
                theme::result_error_hover_text(),
                Some(theme::result_error_active_bg()),
            ),
            (Interaction::Open, false) => (
                theme::result_hover_text(),
                Some(theme::result_value_open_bg()),
            ),
            (Interaction::Open, true) => (
                theme::result_error_hover_text(),
                Some(theme::result_error_active_bg()),
            ),
        }
    }
}

pub struct ResultElement {
    text: SharedString,
    repetend: Option<Range<usize>>,
    color: Hsla,
    background: Option<Hsla>,
    font: Font,
    font_size: Pixels,
    /// Paint-only scale; layout remains unscaled.
    scale: f32,
    /// Clip width without widening the output column.
    max_width: Pixels,
}

impl ResultElement {
    pub fn new(
        output: &LineOutput,
        budget: usize,
        interaction: Interaction,
        metrics: Metrics,
        fonts: &Fonts,
        max_width: Pixels,
        scale: f32,
    ) -> Self {
        let is_error = matches!(output, LineOutput::Error { .. });
        let (color, background) = interaction.colors(is_error);
        let text = text_for(output, budget);

        Self {
            text: text.text,
            repetend: text.repetend,
            color: theme::hsla(color),
            background: background.map(theme::hsla),
            font: fonts.bold.clone(),
            font_size: metrics.font_size,
            scale,
            max_width,
        }
    }

    fn shape(&self, font_size: Pixels, window: &mut Window) -> ShapedLine {
        let run = TextRun {
            len: self.text.len(),
            font: self.font.clone(),
            color: self.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window
            .text_system()
            .shape_line(self.text.clone(), font_size, &[run], None)
    }
}

impl IntoElement for ResultElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ResultElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<ShapedLine>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let line = self.shape(self.font_size, window);
        let padding = px(2.0 * theme::OUTPUT_PADDING_X);

        let mut style = Style::default();
        style.size.width = (line.width + padding).min(self.max_width).into();
        style.size.height = px(LINE_HEIGHT + 2.0 * theme::OUTPUT_PADDING_Y).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        Some(self.shape(self.font_size * self.scale, window))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        line: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(line) = line.take() else {
            return;
        };

        let scaled = scaled_about_center(bounds, self.scale);

        if let Some(background) = self.background {
            window.paint_quad(gpui::quad(
                scaled,
                gpui::Corners::all(px(theme::OUTPUT_RADIUS * self.scale)),
                background,
                px(0.0),
                gpui::transparent_black(),
                gpui::BorderStyle::default(),
            ));
        }

        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            let line_height = px(LINE_HEIGHT * self.scale);
            let origin = scaled.origin
                + point(
                    px(theme::OUTPUT_PADDING_X * self.scale),
                    px(theme::OUTPUT_PADDING_Y * self.scale),
                );
            line.paint(origin, line_height, window, cx).ok();

            if let Some(repetend) = &self.repetend {
                let top = origin.y + (line_height - line.ascent - line.descent) / 2.0;
                let left = line.x_for_index(repetend.start);
                let right = line.x_for_index(repetend.end);
                window.paint_quad(fill(
                    Bounds::new(
                        point(origin.x + left, top),
                        size(right - left, px(VINCULUM_THICKNESS * self.scale)),
                    ),
                    self.color,
                ));
            }
        });
    }
}

pub struct ValueElement {
    text: SharedString,
    repetend: Option<Range<usize>>,
    color: Hsla,
    font: Font,
    font_size: Pixels,
    line_height: Pixels,
    wrap_width: Pixels,
}

impl ValueElement {
    pub fn new(string: &str, repeating: bool, fonts: &Fonts, wrap_width: Pixels) -> Self {
        let display = repeating
            .then(|| format_repeating_decimal(string))
            .flatten()
            .unwrap_or_else(|| NumberDisplay::plain(string));

        let NumberDisplay {
            before_repeat,
            repeating: repetend,
            after_repeat,
        } = display;
        let start = before_repeat.len();
        let end = start + repetend.len();

        Self {
            text: format!("{before_repeat}{repetend}{after_repeat}").into(),
            repetend: (!repetend.is_empty()).then_some(start..end),
            color: theme::hsla(theme::popover_value()),
            font: fonts.bold.clone(),
            font_size: px(theme::REM),
            line_height: px(theme::ROOT_LINE_HEIGHT),
            wrap_width,
        }
    }

    fn shape(&self, window: &mut Window) -> WrappedLine {
        let run = TextRun {
            len: self.text.len(),
            font: self.font.clone(),
            color: self.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window
            .text_system()
            .shape_text(
                self.text.clone(),
                self.font_size,
                &[run],
                Some(self.wrap_width),
                None,
            )
            .expect("shaping a result cannot fail")
            .into_iter()
            .next()
            .expect("shaping always yields a line")
    }
}

impl IntoElement for ValueElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ValueElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<WrappedLine>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let line = self.shape(window);
        let size = line.size(self.line_height);

        let mut style = Style::default();
        style.size.width = size.width.into();
        style.size.height = size.height.into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        Some(self.shape(window))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        line: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(line) = line.take() else {
            return;
        };

        line.paint(
            bounds.origin,
            self.line_height,
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .ok();

        let Some(repetend) = self.repetend.clone() else {
            return;
        };
        let top = (self.line_height - line.ascent() - line.descent()) / 2.0;
        for rect in row_rects(&line, repetend, self.line_height) {
            window.paint_quad(fill(
                Bounds::new(
                    bounds.origin + rect.origin + point(px(0.0), top),
                    size(rect.width, px(VINCULUM_THICKNESS)),
                ),
                self.color,
            ));
        }
    }
}
