//! Turns markdown source into the flat list of blocks the renderer draws.

use gpui::SharedString;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct SpanStyle {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    /// The destination this run links to, if it is part of a link.
    pub link: Option<SharedString>,
}

#[derive(Clone)]
pub struct Span {
    pub text: String,
    pub style: SpanStyle,
}

#[derive(Clone)]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    Item {
        depth: usize,
        marker: SharedString,
        spans: Vec<Span>,
    },
    Code(SharedString),
    Quote(Vec<Span>),
    Rule,
}

pub fn parse(source: &str) -> Vec<Block> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut state = Builder::default();
    for event in Parser::new_ext(source, options) {
        state.handle(event);
    }
    state.blocks
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    spans: Vec<Span>,
    style: SpanStyle,
    /// The heading level of the block being collected, if it is a heading.
    heading: Option<u8>,
    /// The marker and depth of the list item awaiting its first block of text.
    item: Option<(usize, SharedString)>,
    /// One entry per open list; `Some` holds an ordered list's next number.
    lists: Vec<Option<u64>>,
    quotes: usize,
    code: Option<String>,
}

impl Builder {
    fn handle(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.push_text(&text),
            Event::Code(text) => {
                let style = SpanStyle {
                    code: true,
                    ..self.style.clone()
                };
                self.push_span(text.to_string(), style);
            }
            Event::SoftBreak => self.push_text(" "),
            Event::HardBreak => self.push_text("\n"),
            Event::Rule => {
                self.flush();
                self.blocks.push(Block::Rule);
            }
            Event::TaskListMarker(done) => {
                self.push_text(if done { "☑ " } else { "☐ " });
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = Some(level as u8);
            }
            Tag::Paragraph => self.flush(),
            Tag::List(start) => {
                self.flush();
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush();
                let depth = self.lists.len().saturating_sub(1);
                let marker = match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let marker = SharedString::from(format!("{number}."));
                        *number += 1;
                        marker
                    }
                    _ => SharedString::new_static("•"),
                };
                self.item = Some((depth, marker));
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.quotes += 1;
            }
            Tag::CodeBlock(CodeBlockKind::Fenced(_) | CodeBlockKind::Indented) => {
                self.flush();
                self.code = Some(String::new());
            }
            Tag::Strong => self.style.bold = true,
            Tag::Emphasis => self.style.italic = true,
            Tag::Strikethrough => self.style.strike = true,
            Tag::Link { dest_url, .. } => self.style.link = Some(dest_url.into_string().into()),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::Item => self.flush(),
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quotes = self.quotes.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some(code) = self.code.take() {
                    self.blocks
                        .push(Block::Code(code.trim_end_matches('\n').to_string().into()));
                }
            }
            TagEnd::Strong => self.style.bold = false,
            TagEnd::Emphasis => self.style.italic = false,
            TagEnd::Strikethrough => self.style.strike = false,
            TagEnd::Link => self.style.link = None,
            _ => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        if let Some(code) = self.code.as_mut() {
            code.push_str(text);
            return;
        }
        let style = self.style.clone();
        self.push_span(text.to_string(), style);
    }

    /// Runs that look the same are merged so the renderer draws one highlight
    /// per visible change rather than one per markdown event.
    fn push_span(&mut self, text: String, style: SpanStyle) {
        if text.is_empty() {
            return;
        }
        match self.spans.last_mut() {
            Some(last) if last.style == style => last.text.push_str(&text),
            _ => self.spans.push(Span { text, style }),
        }
    }

    fn flush(&mut self) {
        let spans = std::mem::take(&mut self.spans);
        let heading = self.heading.take();
        let trimmed: Vec<Span> = trim(spans);
        if trimmed.is_empty() {
            return;
        }

        let block = if let Some(level) = heading {
            Block::Heading {
                level,
                spans: trimmed,
            }
        } else if let Some((depth, marker)) = self.item.take() {
            Block::Item {
                depth,
                marker,
                spans: trimmed,
            }
        } else if self.quotes > 0 {
            Block::Quote(trimmed)
        } else {
            Block::Paragraph(trimmed)
        };
        self.blocks.push(block);
    }
}

/// Drops the whitespace markdown leaves at either end of a block.
fn trim(mut spans: Vec<Span>) -> Vec<Span> {
    if let Some(first) = spans.first_mut() {
        first.text = first.text.trim_start().to_string();
    }
    if let Some(last) = spans.last_mut() {
        last.text = last.text.trim_end().to_string();
    }
    spans.retain(|span| !span.text.is_empty());
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|span| span.text.as_str()).collect()
    }

    #[test]
    fn headings_lists_and_code_become_separate_blocks() {
        let blocks =
            parse("## What's new\n\n- Faster **units**\n- Fewer bugs\n\n```\n2 + 2\n```\n");
        assert_eq!(blocks.len(), 4);
        match &blocks[0] {
            Block::Heading { level, spans } => {
                assert_eq!(*level, 2);
                assert_eq!(text(spans), "What's new");
            }
            _ => panic!("expected a heading"),
        }
        match &blocks[1] {
            Block::Item { marker, spans, .. } => {
                assert_eq!(marker.as_ref(), "•");
                assert_eq!(text(spans), "Faster units");
                assert!(spans.iter().any(|span| span.style.bold));
            }
            _ => panic!("expected a list item"),
        }
        assert!(matches!(&blocks[3], Block::Code(code) if code.as_ref() == "2 + 2"));
    }

    #[test]
    fn ordered_items_are_numbered_from_their_own_start() {
        let blocks = parse("3. three\n4. four\n");
        let markers: Vec<_> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Item { marker, .. } => Some(marker.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(markers, vec!["3.", "4."]);
    }

    #[test]
    fn a_loose_item_keeps_its_marker_on_the_first_paragraph_only() {
        let blocks = parse("- first\n\n  second\n");
        assert!(matches!(&blocks[0], Block::Item { .. }));
        assert!(matches!(&blocks[1], Block::Paragraph(_)));
    }

    fn link(span: &Span) -> Option<&str> {
        span.style.link.as_ref().map(SharedString::as_ref)
    }

    #[test]
    fn a_link_carries_its_destination_on_every_run_it_covers() {
        let blocks = parse("See the [**full** notes](https://example.com/notes) online.\n");
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("expected a paragraph");
        };

        let linked: Vec<&Span> = spans.iter().filter(|span| link(span).is_some()).collect();
        let linked_text: String = linked.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(linked_text, "full notes");
        assert!(
            linked
                .iter()
                .all(|span| link(span) == Some("https://example.com/notes"))
        );
        assert_eq!(text(spans), "See the full notes online.");
    }

    #[test]
    fn a_bare_heading_link_still_gets_its_destination() {
        let blocks = parse("## [Release](https://example.com)\n");
        let Block::Heading { spans, .. } = &blocks[0] else {
            panic!("expected a heading");
        };
        assert_eq!(link(&spans[0]), Some("https://example.com"));
    }
}
