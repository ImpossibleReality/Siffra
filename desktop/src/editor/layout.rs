//! Measuring the editor: how wide each column is, and where every line of text
//! ends up once it has been wrapped into the input column.
//!
//! Column widths follow from the window and the separator position, so the text
//! can be shaped up front and the results and rulers use the same measurements.

use crate::highlight::{Span, Token, highlight_line, is_header_line};
use crate::theme;
use gpui::{
    Font, FontWeight, Hsla, Pixels, Point, SharedString, TextRun, Window, WrappedLine, point, px,
};
use std::ops::Range;

#[derive(Clone)]
pub struct Fonts {
    pub regular: Font,
    pub bold: Font,
}

impl Fonts {
    pub fn new(family: impl Into<SharedString>) -> Self {
        let base = gpui::font(family);
        Self {
            regular: Font {
                weight: FontWeight::SEMIBOLD,
                ..base.clone()
            },
            bold: Font {
                weight: FontWeight::BOLD,
                ..base
            },
        }
    }
}

/// Every measurement the editor draws from, derived from the window's width.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub font_size: Pixels,
    pub line_height: Pixels,
    /// How tall the text itself stands, as against the line box holding it.
    /// The caret is drawn to this, not to the line box, which at this line
    /// height would read as a bar down the whole line.
    pub ascent: Pixels,
    pub descent: Pixels,
    /// The advance of one bold digit, which is what a result is set in and so
    /// what its column budget is counted in.
    pub bold_char_width: Pixels,
    pub window_width: Pixels,
    /// Width allotted to results, including its padding.
    pub output_width: Pixels,
    /// Width allotted to input, including its padding.
    pub input_width: Pixels,
    /// The width the input's text wraps at — the input column less its padding.
    pub text_width: Pixels,
}

impl Metrics {
    pub fn new(window: &mut Window, fonts: &Fonts, output_fraction: f32) -> Self {
        let window_width = window.viewport_size().width;
        let font_size = theme::font_size_for_window(window_width);
        let line_height = font_size * theme::LINE_HEIGHT_RATIO;

        let output_width = px((f32::from(window_width) * output_fraction).clamp(
            theme::OUTPUT_MIN_WIDTH,
            (f32::from(window_width) - theme::OUTPUT_MAX_INSET).max(theme::OUTPUT_MIN_WIDTH),
        ));
        let input_width = window_width - output_width - px(theme::SEPARATOR_WIDTH);
        let text_width = (input_width - px(2.0 * theme::GUTTER)).max(px(1.0));

        let text_system = window.text_system().clone();
        let sample = |font: &Font| {
            let run = TextRun {
                len: 1,
                font: font.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            text_system.shape_line("0".into(), font_size, &[run], None)
        };

        let regular = sample(&fonts.regular);
        let bold = sample(&fonts.bold);

        Self {
            font_size,
            line_height,
            ascent: regular.ascent,
            descent: regular.descent,
            bold_char_width: bold.width,
            window_width,
            output_width,
            input_width,
            text_width,
        }
    }

    /// The offset of the text's top from the top of its line box, which is what
    /// the caret and the glyphs are both placed against.
    pub fn half_leading(&self) -> Pixels {
        (self.line_height - self.ascent - self.descent) / 2.0
    }

    /// Whether the window is wide enough for the rulers to be worth drawing.
    pub fn rulers_visible(&self) -> bool {
        f32::from(self.window_width) > theme::RULER_MIN_WINDOW_WIDTH
    }

    /// How many characters of a result the output column can show.
    pub fn output_character_budget(&self) -> usize {
        let row_width = f32::from(self.output_width) - 2.0 * theme::GUTTER;
        let inner = row_width - 2.0 * theme::OUTPUT_PADDING_X;
        let character = f32::from(self.bold_char_width);
        if character <= 0.0 {
            return 1;
        }
        ((inner / character).floor() as isize).max(1) as usize
    }
}

pub struct Line {
    /// Byte offset of the line's first character within the document.
    pub start: usize,
    pub len: usize,
    pub shaped: WrappedLine,
    pub height: Pixels,
    pub is_header: bool,
    /// Whether the line is blank and the selection runs through it. Selection is
    /// carried as a background on the text's runs, and a blank line has no runs
    /// to carry it, so this is the one case that has to be drawn separately.
    pub blank_and_selected: bool,
    /// Where the ruler for this line may start, measured from the text's left
    /// edge. A wrapped line fills the column, leaving no span for the ruler.
    pub ruler_start: Pixels,
}

pub struct Layout {
    pub metrics: Metrics,
    pub lines: Vec<Line>,
    /// Drawn in place of the first line while the editor is empty.
    pub placeholder: Option<WrappedLine>,
    /// Whether any line has wrapped, which is when the separator between the
    /// columns starts to matter and so shows without being hovered.
    pub any_wrapped: bool,
}

impl Layout {
    pub fn new(
        content: &str,
        selection: &Range<usize>,
        metrics: Metrics,
        fonts: &Fonts,
        window: &mut Window,
    ) -> Self {
        let text_system = window.text_system().clone();
        let mut lines = Vec::new();
        let mut any_wrapped = false;
        let mut start = 0;

        for text in content.split('\n') {
            let is_header = is_header_line(text);
            let local_selection = clip(selection, start, text.len());
            let runs = runs_for_line(text, is_header, &local_selection, fonts);

            let shaped = text_system
                .shape_text(
                    SharedString::from(text.to_string()),
                    metrics.font_size,
                    &runs,
                    Some(metrics.text_width),
                    None,
                )
                .expect("shaping a line of input cannot fail")
                .into_iter()
                .next()
                .expect("shaping always yields a line");

            let rows = shaped.wrap_boundaries().len() + 1;
            if rows > 1 {
                any_wrapped = true;
            }

            lines.push(Line {
                start,
                len: text.len(),
                height: metrics.line_height * rows as f32,
                is_header,
                // What is selected on a blank line is the break that ends it,
                // so the selection has to reach past the line's own offset.
                blank_and_selected: text.is_empty()
                    && selection.start <= start
                    && start < selection.end,
                ruler_start: if rows > 1 {
                    metrics.text_width
                } else {
                    shaped.unwrapped_layout.width
                },
                shaped,
            });

            start += text.len() + 1;
        }

        let placeholder = content.is_empty().then(|| {
            let text = SharedString::from("Enter your calculations...");
            let run = TextRun {
                len: text.len(),
                font: fonts.regular.clone(),
                color: theme::hsla(theme::placeholder()),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            text_system
                .shape_text(
                    text,
                    metrics.font_size,
                    &[run],
                    Some(metrics.text_width),
                    None,
                )
                .expect("shaping the placeholder cannot fail")
                .into_iter()
                .next()
                .expect("shaping always yields a line")
        });

        Self {
            metrics,
            lines,
            placeholder,
            any_wrapped,
        }
    }

    /// The height of every line stacked up — what the input column asks for,
    /// and what the results column and the rulers are laid out against.
    pub fn content_height(&self) -> Pixels {
        self.lines
            .iter()
            .fold(px(0.0), |total, line| total + line.height)
    }

    /// The top of `index`'s line, measured from the top of the text.
    pub fn line_top(&self, index: usize) -> Pixels {
        self.lines[..index.min(self.lines.len())]
            .iter()
            .fold(px(0.0), |total, line| total + line.height)
    }

    pub fn line_index_for_offset(&self, offset: usize) -> usize {
        self.lines
            .iter()
            .rposition(|line| line.start <= offset)
            .unwrap_or(0)
    }

    /// Where the caret sits for a document offset, relative to the top left of
    /// the text. `None` only when the offset is outside the document.
    pub fn position_for_offset(&self, offset: usize) -> Option<Point<Pixels>> {
        let index = self.line_index_for_offset(offset);
        let line = self.lines.get(index)?;
        let local = offset.saturating_sub(line.start).min(line.len);
        let position = line
            .shaped
            .position_for_index(local, self.metrics.line_height)?;
        Some(point(position.x, self.line_top(index) + position.y))
    }

    /// The document offset nearest a point, which is what a click resolves to.
    /// Points above the text land at its start and points below it at its end,
    /// as clicking in the slack under a short calculation should.
    pub fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        if position.y < px(0.0) {
            return 0;
        }

        let mut top = px(0.0);
        for line in &self.lines {
            if position.y < top + line.height {
                let local = point(position.x, position.y - top);
                let index = line
                    .shaped
                    .closest_index_for_position(local, self.metrics.line_height)
                    .unwrap_or_else(|index| index);
                return line.start + index.min(line.len);
            }
            top += line.height;
        }

        self.lines
            .last()
            .map(|line| line.start + line.len)
            .unwrap_or(0)
    }
}

/// One visual row's worth of a span of text: where it starts, relative to the
/// top left of the shaped line, and how wide it stands.
#[derive(Clone, Copy, Debug)]
pub struct RowRect {
    pub origin: Point<Pixels>,
    pub width: Pixels,
}

/// The rows a byte range covers within a shaped line.
///
/// A range that runs across a wrap comes back as one rect per row it touches,
/// which is needed for inline backgrounds and overlines.
/// Nothing is measured here that the shaping did not already settle: a row's
/// left edge is the x of its first character in the unwrapped layout, and every
/// x on that row is taken from the same place and offset by it.
pub fn row_rects(shaped: &WrappedLine, range: Range<usize>, line_height: Pixels) -> Vec<RowRect> {
    if range.start >= range.end {
        return Vec::new();
    }

    let unwrapped = &shaped.unwrapped_layout;
    let mut row_starts = vec![0usize];
    row_starts.extend(
        shaped
            .wrap_boundaries
            .iter()
            .map(|boundary| unwrapped.runs[boundary.run_ix].glyphs[boundary.glyph_ix].index),
    );

    let mut rects = Vec::new();
    for (row, &row_start) in row_starts.iter().enumerate() {
        let row_end = row_starts.get(row + 1).copied().unwrap_or(unwrapped.len);
        let from = range.start.max(row_start);
        let to = range.end.min(row_end);
        if from >= to {
            continue;
        }

        // Rows after the first are drawn shifted back to the left margin, so
        // every x on them is measured from where the row itself begins.
        let margin = unwrapped.x_for_index(row_start);
        let left = unwrapped.x_for_index(from) - margin;
        let right = unwrapped.x_for_index(to) - margin;
        if right <= left {
            continue;
        }

        rects.push(RowRect {
            origin: point(left, line_height * row as f32),
            width: right - left,
        });
    }

    rects
}

/// The part of `selection` that falls inside a line, in offsets local to it.
fn clip(selection: &Range<usize>, start: usize, len: usize) -> Range<usize> {
    let end = start + len;
    let from = selection.start.clamp(start, end) - start;
    let to = selection.end.clamp(start, end) - start;
    from..to
}

fn color_for(token: Token) -> Hsla {
    theme::hsla(match token {
        Token::Header => theme::header(),
        Token::Comment => theme::comment(),
        Token::Chemical => theme::chemical(),
        Token::Number => theme::number(),
        Token::Operation => theme::operation(),
    })
}

/// One run per stretch of the line that is coloured the same way and either is
/// or is not selected. Selection is carried as a run background rather than
/// painted separately, so it follows the text across a wrap without any
/// further arithmetic.
fn runs_for_line(
    line: &str,
    is_header: bool,
    selection: &Range<usize>,
    fonts: &Fonts,
) -> Vec<TextRun> {
    let font = if is_header {
        fonts.bold.clone()
    } else {
        fonts.regular.clone()
    };
    let base = theme::hsla(theme::text());
    let selected = theme::hsla(theme::selection());
    let spans = highlight_line(line);

    let mut boundaries = vec![0, line.len()];
    for span in &spans {
        boundaries.push(span.range.start);
        boundaries.push(span.range.end);
    }
    if selection.start != selection.end {
        boundaries.push(selection.start);
        boundaries.push(selection.end);
    }
    boundaries.retain(|offset| *offset <= line.len());
    boundaries.sort_unstable();
    boundaries.dedup();

    let span_at = |offset: usize| -> Option<&Span> {
        spans
            .iter()
            .find(|span| span.range.start <= offset && offset < span.range.end)
    };

    boundaries
        .windows(2)
        .map(|pair| {
            let (start, end) = (pair[0], pair[1]);
            TextRun {
                len: end - start,
                font: font.clone(),
                color: span_at(start).map_or(base, |span| color_for(span.token)),
                background_color: (selection.start <= start
                    && end <= selection.end
                    && selection.start != selection.end)
                    .then_some(selected),
                underline: None,
                strikethrough: None,
            }
        })
        .filter(|run| run.len > 0)
        .collect()
}
