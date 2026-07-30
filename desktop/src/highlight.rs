//! Syntax highlighting for a line of Siffra input.
//!
//! The scanners are explicit because Rust's `regex` does not support the
//! lookbehind needed by some token boundaries. Spans are merged by precedence,
//! so a heading or comment can hide tokens inside it.

use std::ops::Range;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Header,
    Comment,
    Chemical,
    Number,
    Operation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub token: Token,
    pub range: Range<usize>,
}

/// Words the parser reads as operators, so a unit name is never one of them.
/// Longer forms come first: the alternation is ordered, and `divided` would
/// otherwise win over `divided by`.
const OPERATOR_WORDS: &[&str] = &[
    "divided by",
    "divide by",
    "plus",
    "minus",
    "times",
    "of",
    "over",
    "by",
    "divided",
    "divide",
    "in",
    "as",
    "to",
    "into",
    "let",
    "on",
    "off",
    "what",
    "is",
];

/// Words that cannot be the unit name preceding a chemical formula.
const NON_UNIT_WORDS: &[&str] = &[
    "plus", "minus", "times", "of", "over", "by", "divided", "divide", "as", "to", "into", "let",
    "on", "off", "what", "is",
];

const SYMBOL_OPERATORS: &[u8] = b"=+-*/^%[]";

/// Whether `line` is a heading — a line introduced by one or more `#`.
pub fn is_header_line(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Whether `index` lies at a word boundary.
fn at_word_boundary(bytes: &[u8], index: usize) -> bool {
    let before = index > 0 && is_word_byte(bytes[index - 1]);
    let after = index < bytes.len() && is_word_byte(bytes[index]);
    before != after
}

/// Adds a span for each heading's entire line, including leading whitespace.
fn header_spans(text: &str, out: &mut Vec<Span>) {
    let mut start = 0;
    for line in text.split('\n') {
        if is_header_line(line) {
            out.push(Span {
                token: Token::Header,
                range: start..start + line.len(),
            });
        }
        start += line.len() + 1;
    }
}

/// Adds line-comment spans and same-line block-comment spans.
fn comment_spans(text: &str, out: &mut Vec<Span>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() {
            let line_end = text[i..]
                .find('\n')
                .map(|offset| i + offset)
                .unwrap_or(bytes.len());

            if bytes[i + 1] == b'/' {
                out.push(Span {
                    token: Token::Comment,
                    range: i..line_end,
                });
                i = line_end;
                continue;
            }

            if bytes[i + 1] == b'*'
                && let Some(end) = text[i + 2..line_end].rfind("*/")
            {
                let end = i + 2 + end + 2;
                out.push(Span {
                    token: Token::Comment,
                    range: i..end,
                });
                i = end;
                continue;
            }
        }
        i += 1;
    }
}

/// Checks for a standalone alphabetic unit name before a chemical formula, so a
/// formula's own subscripts cannot qualify as that unit.
fn preceded_by_unit_name(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b' ' {
        cursor -= 1;
    }
    if cursor == index {
        return false;
    }

    let word_end = cursor;
    while cursor > 0 && bytes[cursor - 1].is_ascii_alphabetic() {
        cursor -= 1;
    }
    if cursor == word_end {
        return false;
    }
    if cursor > 0 && is_word_byte(bytes[cursor - 1]) {
        return false;
    }

    let word = &bytes[cursor..word_end];
    !NON_UNIT_WORDS
        .iter()
        .any(|candidate| candidate.as_bytes() == word)
}

/// Adds spans for chemical formulas following a unit name.
fn chemical_spans(text: &str, out: &mut Vec<Span>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let starts_formula = bytes[i].is_ascii_uppercase() || bytes[i] == b'(';
        if starts_formula && preceded_by_unit_name(bytes, i) {
            let mut end = i + 1;
            while end < bytes.len()
                && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'(' || bytes[end] == b')')
            {
                end += 1;
            }
            out.push(Span {
                token: Token::Chemical,
                range: i..end,
            });
            i = end;
        } else {
            i += 1;
        }
    }
}

/// Reads comma-grouped digits before underscore-separated digits; the leading
/// comma group is greedy, so `1234,567` remains two plain numbers.
fn digit_run(bytes: &[u8], start: usize) -> Option<usize> {
    if start >= bytes.len() || !bytes[start].is_ascii_digit() {
        return None;
    }

    for leading in (1..=3usize).rev() {
        if start + leading > bytes.len()
            || !bytes[start..start + leading].iter().all(u8::is_ascii_digit)
        {
            continue;
        }

        let mut end = start + leading;
        let mut groups = 0;
        while bytes.get(end) == Some(&b',')
            && bytes
                .get(end + 1..end + 4)
                .is_some_and(|group| group.iter().all(u8::is_ascii_digit))
        {
            end += 4;
            groups += 1;
        }
        if groups > 0 {
            return Some(end);
        }
    }

    let mut end = start + 1;
    loop {
        let next = if bytes.get(end) == Some(&b'_') {
            end + 1
        } else {
            end
        };
        if bytes.get(next).is_some_and(u8::is_ascii_digit) {
            end = next + 1;
        } else {
            return Some(end);
        }
    }
}

/// Reads an optional dollar sign, fraction, and exponent from a number.
fn number_at(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start;
    if bytes.get(cursor) == Some(&b'$') {
        cursor += 1;
    }

    let mut end = digit_run(bytes, cursor)?;

    if bytes.get(end) == Some(&b'.')
        && let Some(fraction_end) = digit_run(bytes, end + 1)
    {
        end = fraction_end;
    }

    if bytes.get(end) == Some(&b'E') {
        let exponent_start = if bytes.get(end + 1) == Some(&b'-') {
            end + 2
        } else {
            end + 1
        };
        if let Some(exponent_end) = digit_run(bytes, exponent_start) {
            end = exponent_end;
        }
    }

    Some(end)
}

fn number_spans(text: &str, out: &mut Vec<Span>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match number_at(bytes, i) {
            Some(end) => {
                out.push(Span {
                    token: Token::Number,
                    range: i..end,
                });
                i = end;
            }
            None => i += 1,
        }
    }
}

fn operation_spans(text: &str, out: &mut Vec<Span>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if SYMBOL_OPERATORS.contains(&bytes[i]) {
            out.push(Span {
                token: Token::Operation,
                range: i..i + 1,
            });
            i += 1;
            continue;
        }

        // A # after text is an operator; headings were claimed as whole lines
        // before operation spans are added.
        if bytes[i] == b'#' && i > 0 && bytes[i - 1] != b'\n' {
            out.push(Span {
                token: Token::Operation,
                range: i..i + 1,
            });
            i += 1;
            continue;
        }

        if at_word_boundary(bytes, i)
            && let Some(word) = OPERATOR_WORDS.iter().find(|word| {
                text[i..].starts_with(**word) && at_word_boundary(bytes, i + word.len())
            })
        {
            out.push(Span {
                token: Token::Operation,
                range: i..i + word.len(),
            });
            i += word.len();
            continue;
        }

        i += 1;
    }
}

/// The tokens of `text`, in order and without overlaps.
///
/// Precedence runs heading, comment, chemical, number, operator: a heading wins
/// over everything on its line, a comment over the formulas and numbers inside
/// it, and so on down. Where two tokens start together the higher-precedence
/// one is kept, and a token that runs into a later one swallows it.
pub fn highlight_line(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    header_spans(text, &mut spans);
    comment_spans(text, &mut spans);
    chemical_spans(text, &mut spans);
    number_spans(text, &mut spans);
    operation_spans(text, &mut spans);

    spans.sort_by_key(|span| span.range.start);

    let mut i = 0;
    while i + 1 < spans.len() {
        if spans[i].range.end > spans[i + 1].range.start {
            spans.remove(i + 1);
        } else {
            i += 1;
        }
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Renders tokens as HTML so the expected spans are easy to inspect.
    fn render(text: &str) -> String {
        let name = |token: Token| match token {
            Token::Header => "header",
            Token::Comment => "comment",
            Token::Chemical => "chemical",
            Token::Number => "number",
            Token::Operation => "operation",
        };

        let mut out = String::new();
        let mut last = 0;
        for span in highlight_line(text) {
            out.push_str(&text[last..span.range.start]);
            out.push_str(&format!(
                "<span class=\"{}\">{}</span>",
                name(span.token),
                &text[span.range.clone()]
            ));
            last = span.range.end;
        }
        out.push_str(&text[last..]);
        out
    }

    #[test]
    fn highlights_currency_prefixed_numbers_and_percentage_symbols() {
        assert_eq!(
            render("$10 + 5%"),
            "<span class=\"number\">$10</span> <span class=\"operation\">+</span> \
             <span class=\"number\">5</span><span class=\"operation\">%</span>"
        );
    }

    #[test]
    fn only_highlights_commas_used_as_three_digit_separators() {
        assert_eq!(
            render("min(1,2) and 1,234"),
            "min(<span class=\"number\">1</span>,<span class=\"number\">2</span>) and \
             <span class=\"number\">1,234</span>"
        );
    }

    #[test]
    fn highlights_percentage_language_and_the_scalar_symbol() {
        assert_eq!(
            render("5% on what is $30 as #"),
            "<span class=\"number\">5</span><span class=\"operation\">%</span> \
             <span class=\"operation\">on</span> <span class=\"operation\">what</span> \
             <span class=\"operation\">is</span> <span class=\"number\">$30</span> \
             <span class=\"operation\">as</span> <span class=\"operation\">#</span>"
        );
    }

    #[test]
    fn highlights_brackets_around_unit_expressions() {
        assert_eq!(
            render("5 [kg * m / s^2]"),
            "<span class=\"number\">5</span> <span class=\"operation\">[</span>kg \
             <span class=\"operation\">*</span> m <span class=\"operation\">/</span> \
             s<span class=\"operation\">^</span><span class=\"number\">2</span>\
             <span class=\"operation\">]</span>"
        );
    }

    #[test]
    fn highlights_chemical_qualifiers_in_bare_and_bracketed_units() {
        assert_eq!(
            render("18 [g H2O] as [mol Ca(NO3)2]"),
            "<span class=\"number\">18</span> <span class=\"operation\">[</span>g \
             <span class=\"chemical\">H2O</span><span class=\"operation\">]</span> \
             <span class=\"operation\">as</span> <span class=\"operation\">[</span>mol \
             <span class=\"chemical\">Ca(NO3)2</span><span class=\"operation\">]</span>"
        );
    }

    #[test]
    fn only_treats_formulas_following_unit_names_as_chemicals() {
        assert_eq!(
            render("H2O + NaCl // what is H2O"),
            "H<span class=\"number\">2</span>O <span class=\"operation\">+</span> NaCl \
             <span class=\"comment\">// what is H2O</span>"
        );
        assert_eq!(
            render("what is H2O"),
            "<span class=\"operation\">what</span> <span class=\"operation\">is</span> \
             H<span class=\"number\">2</span>O"
        );
    }

    #[test]
    fn highlights_the_scalar_unit_after_a_number() {
        assert_eq!(
            render("5 #"),
            "<span class=\"number\">5</span> <span class=\"operation\">#</span>"
        );
    }

    #[test]
    fn highlights_a_header_on_any_line() {
        assert_eq!(
            render("# Trip budget"),
            "<span class=\"header\"># Trip budget</span>"
        );
        assert_eq!(
            render("5 plus 2\n## Later heading"),
            "<span class=\"number\">5</span> <span class=\"operation\">plus</span> \
             <span class=\"number\">2</span>\n<span class=\"header\">## Later heading</span>"
        );
    }

    #[test]
    fn highlights_every_operator_keyword() {
        for keyword in [
            "plus", "minus", "times", "of", "over", "by", "divided", "divide", "in", "as", "to",
            "into", "let", "on", "off", "what", "is",
        ] {
            assert_eq!(
                render(keyword),
                format!("<span class=\"operation\">{keyword}</span>")
            );
        }
    }

    #[test]
    fn matches_operator_words_only_at_word_boundaries() {
        assert_eq!(render("office island"), "office island");
    }

    #[test]
    fn keeps_percentage_like_text_inside_comments_as_comments() {
        assert_eq!(
            render("// $10 + 5% off"),
            "<span class=\"comment\">// $10 + 5% off</span>"
        );
    }
}
