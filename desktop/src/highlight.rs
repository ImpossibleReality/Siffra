//! Desktop adapter for the parser-owned semantic syntax analysis.

use std::ops::Range;

pub use siffra::parser::SyntaxKind as Token;
use siffra::parser::syntax_spans;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub token: Token,
    pub range: Range<usize>,
}

/// Whether `line` is a heading — a line introduced by one or more `#`.
pub fn is_header_line(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// Semantic syntax spans for a line, including useful partial output when the
/// expression is unfinished or invalid.
pub fn highlight_line(text: &str) -> Vec<Span> {
    syntax_spans(text)
        .into_iter()
        .map(|span| Span {
            token: span.kind,
            range: span.span.start()..span.span.end(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(text: &str) -> String {
        let name = |token: Token| match token {
            Token::Header => "header",
            Token::Comment => "comment",
            Token::Chemical => "chemical",
            Token::Number => "number",
            Token::Unit => "unit",
            Token::Function => "function",
            Token::Variable => "variable",
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
    fn highlights_variables_numbers_units_and_chemicals_separately() {
        assert_eq!(
            render("mass = 18 [g H2O / mol] + offset"),
            "<span class=\"variable\">mass</span> \
             <span class=\"operation\">=</span> <span class=\"number\">18</span> \
             <span class=\"operation\">[</span><span class=\"unit\">g</span> \
             <span class=\"chemical\">H2O</span> <span class=\"operation\">/</span> \
             <span class=\"unit\">mol</span><span class=\"operation\">]</span> \
             <span class=\"operation\">+</span> <span class=\"variable\">offset</span>"
        );
    }

    #[test]
    fn preserves_highlights_on_incomplete_input() {
        assert_eq!(
            render("total = 18 [g H2O /"),
            "<span class=\"variable\">total</span> \
             <span class=\"operation\">=</span> <span class=\"number\">18</span> \
             <span class=\"operation\">[</span><span class=\"unit\">g</span> \
             <span class=\"chemical\">H2O</span> <span class=\"operation\">/</span>"
        );
    }

    #[test]
    fn parser_context_disambiguates_inches_and_conversion() {
        assert_eq!(
            render("3 in in cm"),
            "<span class=\"number\">3</span> <span class=\"unit\">in</span> \
             <span class=\"operation\">in</span> <span class=\"unit\">cm</span>"
        );
    }

    #[test]
    fn comments_and_headers_keep_precedence() {
        assert_eq!(
            render("// $10 + 5% off"),
            "<span class=\"comment\">// $10 + 5% off</span>"
        );
        assert_eq!(
            render("# Trip budget"),
            "<span class=\"header\"># Trip budget</span>"
        );
    }

    #[test]
    fn highlights_functions_and_grouped_numbers() {
        assert_eq!(
            render("round(1,234.5e-2)"),
            "<span class=\"function\">round</span>(<span class=\"number\">1,234.5e-2</span>)"
        );
    }
}
