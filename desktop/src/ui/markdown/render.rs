//! Draws the blocks produced by [`super::parse`].

use super::parse::{Block, Span, SpanStyle};
use crate::ui::theme;
use gpui::{
    AnyElement, ElementId, FontStyle, FontWeight, HighlightStyle, InteractiveText, IntoElement,
    ParentElement, Pixels, SharedString, StrikethroughStyle, Styled, StyledText, UnderlineStyle,
    div, px,
};
use std::ops::Range;

/// The type the markdown is drawn at, and the family its code is set in.
#[derive(Clone)]
pub struct MarkdownStyle {
    pub font_size: Pixels,
    pub line_height: Pixels,
    pub mono_family: SharedString,
}

impl MarkdownStyle {
    pub fn new(font_size: Pixels, mono_family: impl Into<SharedString>) -> Self {
        Self {
            font_size,
            line_height: font_size * 1.5,
            mono_family: mono_family.into(),
        }
    }
}

/// Renders parsed markdown as a column of blocks.
///
/// The id distinguishes this markdown from any other on screen; each block
/// takes an id under it so links stay addressable.
pub fn markdown(
    id: impl Into<SharedString>,
    blocks: &[Block],
    style: &MarkdownStyle,
) -> AnyElement {
    let id = id.into();
    div()
        .flex()
        .flex_col()
        .gap(px(theme::MARKDOWN_BLOCK_GAP))
        .text_size(style.font_size)
        .line_height(style.line_height)
        .text_color(theme::markdown_text())
        .children(
            blocks
                .iter()
                .enumerate()
                .map(|(index, block)| block_element((id.clone(), index).into(), block, style)),
        )
        .into_any_element()
}

fn block_element(id: ElementId, block: &Block, style: &MarkdownStyle) -> AnyElement {
    match block {
        Block::Heading { level, spans } => {
            let scale = match level {
                1 => 1.35,
                2 => 1.18,
                3 => 1.05,
                _ => 0.95,
            };
            div()
                .text_size(style.font_size * scale)
                .line_height(style.font_size * scale * 1.35)
                .font_weight(FontWeight::BOLD)
                .text_color(theme::markdown_heading())
                .child(styled(id, spans))
                .into_any_element()
        }
        Block::Paragraph(spans) => div().child(styled(id, spans)).into_any_element(),
        Block::Item {
            depth,
            marker,
            spans,
        } => div()
            .flex()
            .items_start()
            .gap(px(0.5 * theme::REM))
            .pl(px(*depth as f32 * theme::MARKDOWN_INDENT))
            .child(
                div()
                    .flex_none()
                    .min_w(px(theme::MARKDOWN_INDENT))
                    .text_color(theme::markdown_quote())
                    .child(marker.clone()),
            )
            .child(div().flex_1().child(styled(id, spans)))
            .into_any_element(),
        Block::Code(code) => div()
            .w_full()
            .px(px(0.7 * theme::REM))
            .py(px(0.5 * theme::REM))
            .bg(theme::markdown_code_bg())
            .border_1()
            .border_color(theme::markdown_code_border())
            .rounded(px(0.4 * theme::REM))
            .font_family(style.mono_family.clone())
            .text_size(style.font_size * 0.9)
            .text_color(theme::markdown_code())
            .children(
                code.split('\n')
                    .map(|line| div().child(SharedString::from(line.to_string()))),
            )
            .into_any_element(),
        Block::Quote(spans) => div()
            .pl(px(0.7 * theme::REM))
            .border_l(px(theme::MARKDOWN_QUOTE_BAR))
            .border_color(theme::markdown_quote())
            .child(styled(id, spans))
            .into_any_element(),
        Block::Rule => div()
            .w_full()
            .my(px(theme::MARKDOWN_RULE_MARGIN))
            .h(px(1.0))
            .bg(theme::markdown_rule())
            .into_any_element(),
    }
}

/// Joins a block's spans into one run of text with a highlight per span.
///
/// Links become clickable ranges over that same text, so an
/// [`InteractiveText`] can open them without the text being split into
/// separate elements that would each wrap on their own.
fn styled(id: ElementId, spans: &[Span]) -> AnyElement {
    let mut text = String::new();
    let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    let mut links: Vec<(Range<usize>, SharedString)> = Vec::new();
    for span in spans {
        let start = text.len();
        text.push_str(&span.text);
        let range = start..text.len();
        if let Some(highlight) = highlight(&span.style) {
            highlights.push((range.clone(), highlight));
        }
        if let Some(url) = span.style.link.as_ref() {
            match links.last_mut() {
                // Emphasis inside a link splits it into several spans, which
                // are one clickable region as far as the reader is concerned.
                Some((last, previous)) if last.end == range.start && previous == url => {
                    last.end = range.end
                }
                _ => links.push((range, url.clone())),
            }
        }
    }

    let text = StyledText::new(SharedString::from(text)).with_highlights(highlights);
    if links.is_empty() {
        return text.into_any_element();
    }

    let (ranges, urls): (Vec<_>, Vec<_>) = links.into_iter().unzip();
    InteractiveText::new(id, text)
        .on_click(ranges, move |index, _, cx| {
            if let Some(url) = urls.get(index) {
                cx.open_url(url);
            }
        })
        .into_any_element()
}

fn highlight(span: &SpanStyle) -> Option<HighlightStyle> {
    if span == &SpanStyle::default() {
        return None;
    }
    let link = span.link.is_some();
    Some(HighlightStyle {
        color: if span.code {
            Some(theme::markdown_code().into())
        } else if link {
            Some(theme::markdown_link().into())
        } else if span.bold {
            Some(theme::markdown_heading().into())
        } else {
            None
        },
        background_color: span.code.then(|| theme::markdown_code_bg().into()),
        font_weight: span.bold.then_some(FontWeight::BOLD),
        font_style: span.italic.then_some(FontStyle::Italic),
        underline: link.then_some(UnderlineStyle {
            thickness: px(1.0),
            color: Some(theme::markdown_link().into()),
            wavy: false,
        }),
        strikethrough: span.strike.then_some(StrikethroughStyle {
            thickness: px(1.0),
            color: Some(theme::markdown_text().into()),
        }),
        ..Default::default()
    })
}
