//! One line's result, in the column on the right.
//!
//! A result is shown at the most precision the column has room for and no more,
//! so the number stays readable as the columns are dragged about. Lines that
//! produce nothing — blanks, comments, headings — contribute an empty row, which
//! is what keeps the column in step with the input beside it.
//!
//! The text is painted rather than handed to a `div`, for the sake of the
//! vinculum: a recurring decimal is written with a rule over its repetend, and
//! there is no overline to ask for. Painting the rule means it can be placed on
//! the font's own ascent and take its
//! colour from the same place the glyphs do, so it follows the text through a
//! hover.

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

/// How a result reads, and which part of it recurs.
struct Text {
    text: SharedString,
    repetend: Option<Range<usize>>,
    characters: usize,
}

fn text_for(output: &LineOutput, budget: usize) -> Text {
    match output {
        LineOutput::Value { string, repeating } => {
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

/// How wide the result stands, which determines where the ruler stops.
pub fn width(output: &LineOutput, budget: usize, metrics: Metrics) -> Pixels {
    metrics.bold_char_width * text_for(output, budget).characters as f32
        + px(2.0 * theme::OUTPUT_PADDING_X)
}

/// Scales a box around its center without moving that center.
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

/// The state used to choose a result's colors and painted vinculum.
///
/// The variants are in the order their rules were written, which is the order
/// they outrank each other in: an open result keeps its colour however it is
/// being pointed at, and one that has just been copied holds the pressed colour
/// even after the button has come back up.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Interaction {
    Idle,
    Hovered,
    /// Pressed or recently copied.
    Pressed,
    /// The result's popover is showing.
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
            // An open error keeps error styling rather than taking the value
            // color used by an open result.
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
    /// How far into the copy squish the result is. Scaling affects only paint,
    /// so the laid-out width remains available to the ruler.
    scale: f32,
    /// The result never pushes the column wider than it is; it clips instead.
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
        // Measured unscaled: the squish is a transform, and a transform never
        // moves the ruler that stops short of this result.
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

        // Keep the row fixed while the result shrinks toward its center.
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

        // Clip an oversized result to its laid-out box instead of widening the
        // column; the copy squish stays inside that same box.
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            let line_height = px(LINE_HEIGHT * self.scale);
            let origin = scaled.origin
                + point(
                    px(theme::OUTPUT_PADDING_X * self.scale),
                    px(theme::OUTPUT_PADDING_Y * self.scale),
                );
            line.paint(origin, line_height, window, cx).ok();

            // The rule sits on the font's ascent, which is where an overline is
            // drawn, and runs the width of the digits that recur.
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

/// The result at full precision, as the popover shows it.
///
/// This is the same number the column above holds, minus the truncation — the
/// popover exists to say what was not shown. It wraps rather than clips, and its
/// recurring part still wants a rule over it, which is why this is painted for
/// the same reason [`ResultElement`] is.
pub struct ValueElement {
    text: SharedString,
    repetend: Option<Range<usize>>,
    color: Hsla,
    font: Font,
    font_size: Pixels,
    line_height: Pixels,
    /// What the text may run to before it wraps, which is also the widest the
    /// popover holding it can be.
    wrap_width: Pixels,
}

impl ValueElement {
    /// The full text of a value, split at its recurring part. A value with no
    /// repetend is simply all `before_repeat`.
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
        // A short value asks for no more room than it needs and lets the
        // popover shrink to it; a long one wraps at the width above and takes
        // as many rows as that comes to.
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

        // Paint one rule for each visual row covered by the repetend.
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
