use crate::syntax::{Span, Token, highlight_line, is_header_line};
use crate::ui::theme;
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

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub font_size: Pixels,
    pub line_height: Pixels,
    pub ascent: Pixels,
    pub descent: Pixels,
    pub bold_char_width: Pixels,
    pub window_width: Pixels,
    pub output_width: Pixels,
    pub input_width: Pixels,
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

    pub fn half_leading(&self) -> Pixels {
        (self.line_height - self.ascent - self.descent) / 2.0
    }

    pub fn rulers_visible(&self) -> bool {
        f32::from(self.window_width) > theme::RULER_MIN_WINDOW_WIDTH
    }

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
    pub start: usize,
    pub len: usize,
    pub shaped: WrappedLine,
    pub height: Pixels,
    pub is_header: bool,
    pub blank_and_selected: bool,
    pub ruler_start: Pixels,
}

pub struct Layout {
    pub metrics: Metrics,
    pub lines: Vec<Line>,
    pub placeholder: Option<WrappedLine>,
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

    pub fn content_height(&self) -> Pixels {
        self.lines
            .iter()
            .fold(px(0.0), |total, line| total + line.height)
    }

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

    pub fn position_for_offset(&self, offset: usize) -> Option<Point<Pixels>> {
        let index = self.line_index_for_offset(offset);
        let line = self.lines.get(index)?;
        let local = offset.saturating_sub(line.start).min(line.len);
        let position = line
            .shaped
            .position_for_index(local, self.metrics.line_height)?;
        Some(point(position.x, self.line_top(index) + position.y))
    }

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

#[derive(Clone, Copy, Debug)]
pub struct RowRect {
    pub origin: Point<Pixels>,
    pub width: Pixels,
}

/// Returns one rectangle per visual row touched by `range`.
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
        Token::Unit => theme::unit(),
        Token::Function => theme::operation(),
        Token::Variable => theme::variable(),
        Token::Operation => theme::operation(),
    })
}

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
