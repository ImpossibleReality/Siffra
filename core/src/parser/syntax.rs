//! Lossless, tolerant syntax analysis for editor highlighting.
//!
//! Evaluation deliberately requires a complete [`ParsedLine`](super::ParsedLine).
//! Highlighting has a different contract: every character must remain usable
//! while the user is halfway through an expression. This module therefore uses
//! Chumsky as a lossless lexer, enriches successful parses with their semantic
//! AST spans, and applies small context-aware recoveries for unfinished unit
//! expressions. The fallback token consumes exactly one character, so this
//! analysis always produces output.

use super::{
    KNOWN_FUNCS, ParsedDimension, ParsedExpr, ParsedLine, ParsedUnit, Span, UNIT_EXCLUDE,
    chemical_parser, number_parser, parse_line,
};
use chumsky::prelude::*;
use std::ops::Range;

type LexExtra<'a> = extra::Err<Rich<'a, char>>;
type LexParser<'a, O> = chumsky::Boxed<'a, 'a, &'a str, O, LexExtra<'a>>;

/// A semantic category understood by editor themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxKind {
    Header,
    Comment,
    Chemical,
    Number,
    Unit,
    Function,
    Variable,
    Operation,
}

/// One semantic syntax range. Offsets are UTF-8 byte offsets, matching Rust
/// string slices and the desktop text renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxSpan {
    pub kind: SyntaxKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexKind {
    Whitespace,
    Number,
    Identifier,
    Comment,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LexToken {
    kind: LexKind,
    range: Range<usize>,
}

const OPERATOR_WORDS: &[&str] = &[
    "divided", "divide", "plus", "minus", "times", "of", "over", "by", "in", "as", "to", "into",
    "let", "on", "off", "what", "is",
];

const OPERATOR_SYMBOLS: &str = "=+-*/^%[]!#";

fn lex_parser<'a>() -> impl Parser<'a, &'a str, Vec<LexToken>, LexExtra<'a>> {
    let line_comment: LexParser<'a, LexKind> = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .to(LexKind::Comment)
        .boxed();

    // Stop immediately before the first `*/`, but accept end-of-input as the
    // natural recovered form of a block comment still being typed.
    let block_comment: LexParser<'a, LexKind> = just("/*")
        .then(any().and_is(just("*/").not()).repeated())
        .then(just("*/").or_not())
        .to(LexKind::Comment)
        .boxed();

    let whitespace: LexParser<'a, LexKind> = any()
        .filter(|c: &char| c.is_whitespace())
        .repeated()
        .at_least(1)
        .to(LexKind::Whitespace)
        .boxed();

    let dollar_number: LexParser<'a, LexKind> =
        just('$').then(number_parser()).to(LexKind::Number).boxed();
    let number: LexParser<'a, LexKind> = number_parser().to(LexKind::Number).boxed();

    let degree_identifier: LexParser<'a, LexKind> = just('°')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphabetic())
                .repeated()
                .at_least(1),
        )
        .to(LexKind::Identifier)
        .boxed();
    let identifier: LexParser<'a, LexKind> = text::ascii::ident().to(LexKind::Identifier).boxed();

    choice((
        line_comment,
        block_comment,
        whitespace,
        dollar_number,
        number,
        degree_identifier,
        identifier,
        any().to(LexKind::Symbol),
    ))
    .map_with(|kind, e| {
        let span: SimpleSpan = e.span();
        LexToken {
            kind,
            range: span.start..span.end,
        }
    })
    .repeated()
    .collect::<Vec<_>>()
    .then_ignore(end())
}

fn lex(input: &str) -> Vec<LexToken> {
    lex_parser()
        .parse(input)
        .into_output()
        .expect("the lossless syntax lexer always consumes every character")
}

fn token_text<'a>(input: &'a str, token: &LexToken) -> &'a str {
    &input[token.range.clone()]
}

fn next_significant(tokens: &[LexToken], mut index: usize) -> Option<usize> {
    while let Some(token) = tokens.get(index) {
        if token.kind != LexKind::Whitespace {
            return (token.kind != LexKind::Comment).then_some(index);
        }
        index += 1;
    }
    None
}

fn is_operator_word(word: &str) -> bool {
    OPERATOR_WORDS.contains(&word)
}

fn looks_like_function(input: &str, tokens: &[LexToken], index: usize) -> bool {
    let word = token_text(input, &tokens[index]);
    let Some(next) = next_significant(tokens, index + 1) else {
        return false;
    };
    let next_text = token_text(input, &tokens[next]);
    next_text == "("
        || (KNOWN_FUNCS.contains(&word) && tokens[index].range.end < tokens[next].range.start)
}

fn syntax(kind: SyntaxKind, range: Range<usize>) -> SyntaxSpan {
    SyntaxSpan {
        kind,
        span: Span::new(range.start, range.end),
    }
}

fn base_spans(input: &str, tokens: &[LexToken]) -> Vec<SyntaxSpan> {
    let mut spans = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let text = token_text(input, token);
        let kind = match token.kind {
            LexKind::Whitespace => None,
            LexKind::Comment => Some(SyntaxKind::Comment),
            LexKind::Number => Some(SyntaxKind::Number),
            LexKind::Identifier if is_operator_word(text) => Some(SyntaxKind::Operation),
            LexKind::Identifier if looks_like_function(input, tokens, index) => {
                Some(SyntaxKind::Function)
            }
            LexKind::Identifier => Some(SyntaxKind::Variable),
            LexKind::Symbol if text.chars().all(|c| OPERATOR_SYMBOLS.contains(c)) => {
                Some(SyntaxKind::Operation)
            }
            LexKind::Symbol => None,
        };
        if let Some(kind) = kind {
            spans.push(syntax(kind, token.range.clone()));
        }
    }
    spans
}

fn header_spans(input: &str, out: &mut Vec<SyntaxSpan>) {
    let mut start = 0;
    for line in input.split_inclusive('\n') {
        let content = line.strip_suffix('\n').unwrap_or(line);
        if content.trim_start().starts_with('#') {
            out.push(syntax(SyntaxKind::Header, start..start + content.len()));
        }
        start += line.len();
    }
}

fn add_unit(input: &str, unit: &ParsedUnit, out: &mut Vec<SyntaxSpan>) {
    let start = unit.span.start();
    let name_end = start.saturating_add(unit.name.len());
    if input
        .get(start..name_end)
        .is_some_and(|source| source == unit.name)
    {
        out.push(syntax(SyntaxKind::Unit, start..name_end));
    } else if unit.name == "percent" && input.get(start..unit.span.end()) == Some("%") {
        // The parser normalizes the symbolic spelling to `percent` for
        // evaluation, but highlighting must retain its source range.
        out.push(syntax(SyntaxKind::Unit, start..unit.span.end()));
    }

    if let Some(chemical) = &unit.chemical {
        let end = unit.span.end();
        let chemical_start = end.saturating_sub(chemical.len());
        if input
            .get(chemical_start..end)
            .is_some_and(|source| source == chemical)
        {
            out.push(syntax(SyntaxKind::Chemical, chemical_start..end));
        }
    }
}

fn add_dimension(input: &str, dimension: &ParsedDimension, out: &mut Vec<SyntaxSpan>) {
    for (unit, _) in dimension.numerator.iter().chain(&dimension.denominator) {
        add_unit(input, unit, out);
    }
}

fn add_expr(input: &str, expr: &ParsedExpr, out: &mut Vec<SyntaxSpan>) {
    match expr {
        ParsedExpr::Number { units, .. } => {
            if let Some(units) = units {
                add_dimension(input, units, out);
            }
        }
        ParsedExpr::Variable { span, .. } => {
            out.push(syntax(SyntaxKind::Variable, span.start()..span.end()));
        }
        ParsedExpr::FunctionCall {
            args,
            base,
            function_span,
            ..
        } => {
            out.push(syntax(
                SyntaxKind::Function,
                function_span.start()..function_span.end(),
            ));
            if let Some(base) = base {
                add_expr(input, base, out);
            }
            for arg in args {
                add_expr(input, arg, out);
            }
        }
        ParsedExpr::UnOpPre { rhs, .. } => add_expr(input, rhs, out),
        ParsedExpr::UnOpPost { lhs, op, .. } => {
            add_expr(input, lhs, out);
            if let super::OpPost::Convert(dimension) = op {
                add_dimension(input, dimension, out);
            }
        }
        ParsedExpr::BinOp { lhs, rhs, .. } => {
            add_expr(input, lhs, out);
            add_expr(input, rhs, out);
        }
    }
}

fn add_parsed_spans(input: &str, out: &mut Vec<SyntaxSpan>) -> bool {
    match parse_line(input) {
        Ok(ParsedLine::Expression(expr)) => add_expr(input, &expr, out),
        Ok(ParsedLine::Variable(_, expr)) => add_expr(input, &expr, out),
        Ok(ParsedLine::Comment | ParsedLine::Header(_)) => {}
        Err(_) => return false,
    }
    true
}

fn is_unit_name(text: &str) -> bool {
    text.strip_prefix('°')
        .unwrap_or(text)
        .chars()
        .all(|c| c.is_ascii_alphabetic())
        && text != "°"
        && !UNIT_EXCLUDE.contains(&text)
}

fn chemical_prefix(input: &str, start: usize) -> Option<Range<usize>> {
    chemical_parser()
        .map_with(|_, e| {
            let span: SimpleSpan = e.span();
            start + span.start..start + span.end
        })
        // `Parser::parse` requires the full input. Consume the suffix after
        // capturing the chemical's greedy prefix so this remains a local
        // recovery rather than a second whole-line grammar.
        .then_ignore(any().repeated())
        .parse(input.get(start..input.len())?)
        .into_output()
}

fn token_covering(tokens: &[LexToken], offset: usize) -> Option<usize> {
    tokens
        .iter()
        .position(|token| token.range.start <= offset && offset < token.range.end)
}

fn add_chemical_after_unit(
    input: &str,
    tokens: &[LexToken],
    unit_index: usize,
    out: &mut Vec<SyntaxSpan>,
) -> Option<usize> {
    let whitespace = tokens.get(unit_index + 1)?;
    if whitespace.kind != LexKind::Whitespace {
        return None;
    }
    let formula_index = next_significant(tokens, unit_index + 1)?;
    let formula_start = tokens[formula_index].range.start;
    let range = chemical_prefix(input, formula_start)?;
    out.push(syntax(SyntaxKind::Chemical, range.clone()));
    token_covering(tokens, range.end.saturating_sub(1))
}

/// Marks a unit expression beginning at `start`. This is intentionally
/// forgiving: missing powers, terms, or a closing bracket simply end the
/// recovered expression rather than invalidating earlier unit names.
fn add_tolerant_dimension(
    input: &str,
    tokens: &[LexToken],
    start: usize,
    out: &mut Vec<SyntaxSpan>,
) {
    let Some(first) = next_significant(tokens, start) else {
        return;
    };
    let bracketed = token_text(input, &tokens[first]) == "[";
    let mut index = if bracketed { first + 1 } else { first };
    let mut expect_unit = true;

    while let Some(current) = next_significant(tokens, index) {
        let token = &tokens[current];
        if token.kind == LexKind::Comment {
            break;
        }
        let text = token_text(input, token);

        if bracketed && text == "]" {
            break;
        }

        if expect_unit {
            if token.kind == LexKind::Identifier && is_unit_name(text) {
                out.push(syntax(SyntaxKind::Unit, token.range.clone()));
                index = add_chemical_after_unit(input, tokens, current, out)
                    .map_or(current + 1, |chemical_end| chemical_end + 1);
                expect_unit = false;
                continue;
            }
            if matches!(text, "%" | "#") {
                out.push(syntax(SyntaxKind::Unit, token.range.clone()));
                index = current + 1;
                expect_unit = false;
                continue;
            }
            // `1/s` is the denominator-only unit form. The one remains a
            // numeric literal; the slash puts the scanner back in unit mode.
            if text == "1" {
                index = current + 1;
                expect_unit = false;
                continue;
            }
            break;
        }

        if text == "^" {
            index = current + 1;
            if let Some(sign_or_power) = next_significant(tokens, index) {
                let power_text = token_text(input, &tokens[sign_or_power]);
                index = if power_text == "-" {
                    sign_or_power + 1
                } else {
                    sign_or_power
                };
                if let Some(power) = next_significant(tokens, index)
                    && tokens[power].kind == LexKind::Number
                {
                    index = power + 1;
                }
            }
            continue;
        }

        if matches!(text, "*" | "/") {
            if !bracketed {
                let previous_end = tokens
                    .get(current.wrapping_sub(1))
                    .map_or(token.range.start, |previous| previous.range.end);
                if previous_end != token.range.start {
                    break;
                }
                let Some(next) = tokens.get(current + 1) else {
                    break;
                };
                if next.range.start != token.range.end {
                    break;
                }
            }
            index = current + 1;
            expect_unit = true;
            continue;
        }

        break;
    }
}

fn add_tolerant_unit_spans(input: &str, tokens: &[LexToken], out: &mut Vec<SyntaxSpan>) {
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == LexKind::Number
            && !token_text(input, token).starts_with('$')
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.kind == LexKind::Whitespace)
        {
            add_tolerant_dimension(input, tokens, index + 1, out);
        }

        if token.kind == LexKind::Identifier
            && matches!(token_text(input, token), "in" | "as" | "to" | "into")
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.kind == LexKind::Whitespace)
        {
            add_tolerant_dimension(input, tokens, index + 1, out);
        }

        if token_text(input, token) == "-"
            && tokens
                .get(index + 1)
                .is_some_and(|next| token_text(input, next) == ">")
        {
            add_tolerant_dimension(input, tokens, index + 2, out);
        }
    }
}

fn priority(kind: SyntaxKind) -> u8 {
    match kind {
        SyntaxKind::Header => 8,
        SyntaxKind::Comment => 7,
        SyntaxKind::Chemical => 6,
        SyntaxKind::Unit => 5,
        SyntaxKind::Number => 4,
        SyntaxKind::Function => 3,
        SyntaxKind::Variable => 2,
        SyntaxKind::Operation => 1,
    }
}

fn normalize(input: &str, spans: Vec<SyntaxSpan>) -> Vec<SyntaxSpan> {
    let mut boundaries = vec![0, input.len()];
    for span in &spans {
        boundaries.push(span.span.start());
        boundaries.push(span.span.end());
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut output: Vec<SyntaxSpan> = Vec::new();
    for pair in boundaries.windows(2) {
        let range = pair[0]..pair[1];
        if range.is_empty() {
            continue;
        }
        let kind = spans
            .iter()
            .filter(|span| span.span.start() <= range.start && range.end <= span.span.end())
            .max_by_key(|span| priority(span.kind))
            .map(|span| span.kind);
        let Some(kind) = kind else {
            continue;
        };

        if let Some(previous) = output.last_mut()
            && previous.kind == kind
            && previous.span.end() == range.start
        {
            previous.span = Span::new(previous.span.start(), range.end);
        } else {
            output.push(syntax(kind, range));
        }
    }
    output
}

/// Returns semantic syntax spans even when `input` is incomplete or invalid.
/// Successful parses provide authoritative variable, function, unit, and
/// chemical roles; the tolerant unit pass preserves those roles while typing.
pub fn syntax_spans(input: &str) -> Vec<SyntaxSpan> {
    let tokens = lex(input);
    let mut spans = base_spans(input, &tokens);
    header_spans(input, &mut spans);
    if !add_parsed_spans(input, &mut spans) {
        add_tolerant_unit_spans(input, &tokens, &mut spans);
    }
    normalize(input, spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(input: &str) -> String {
        let mut out = String::new();
        let mut last = 0;
        for span in syntax_spans(input) {
            let range = span.span.start()..span.span.end();
            out.push_str(&input[last..range.start]);
            out.push_str(&format!(
                "<{:?}>{}</{:?}>",
                span.kind, &input[range], span.kind
            ));
            last = span.span.end();
        }
        out.push_str(&input[last..]);
        out
    }

    #[test]
    fn distinguishes_semantic_leaf_kinds() {
        assert_eq!(
            render("mass = 18 [g H2O / mol] + offset"),
            "<Variable>mass</Variable> <Operation>=</Operation> <Number>18</Number> \
             <Operation>[</Operation><Unit>g</Unit> <Chemical>H2O</Chemical> \
             <Operation>/</Operation> <Unit>mol</Unit><Operation>]</Operation> \
             <Operation>+</Operation> <Variable>offset</Variable>"
        );
    }

    #[test]
    fn uses_context_to_disambiguate_inches_from_conversion() {
        assert_eq!(
            render("3 in in cm"),
            "<Number>3</Number> <Unit>in</Unit> <Operation>in</Operation> <Unit>cm</Unit>"
        );
    }

    #[test]
    fn preserves_semantics_on_incomplete_lines() {
        for input in ["total = 18 [g H2O /", "5 kg*m/s^", "5 m +", "sin("] {
            let spans = syntax_spans(input);
            assert!(!spans.is_empty(), "{input:?} should retain syntax spans");
        }

        assert!(render("total = 18 [g H2O /").contains("<Unit>g</Unit>"));
        assert!(render("total = 18 [g H2O /").contains("<Chemical>H2O</Chemical>"));
        assert!(render("5 kg*m/s^").contains("<Unit>s</Unit>"));
        assert_eq!(
            render("5 plus"),
            "<Number>5</Number> <Operation>plus</Operation>"
        );
    }

    #[test]
    fn comments_override_tokens_inside_them() {
        assert_eq!(
            render("5 m // 20 kg H2O"),
            "<Number>5</Number> <Unit>m</Unit> <Comment>// 20 kg H2O</Comment>"
        );
    }

    #[test]
    fn shares_grouped_decimal_and_exponent_rules_with_evaluation() {
        assert_eq!(
            render("1,234.5e-2 + .25"),
            "<Number>1,234.5e-2</Number> <Operation>+</Operation> <Number>.25</Number>"
        );
    }

    #[test]
    fn distinguishes_percent_operators_from_symbolic_units() {
        assert_eq!(
            render("5% + 5 % + 5 #"),
            "<Number>5</Number><Operation>%</Operation> <Operation>+</Operation> \
             <Number>5</Number> <Unit>%</Unit> <Operation>+</Operation> \
             <Number>5</Number> <Unit>#</Unit>"
        );
    }

    #[test]
    fn byte_spans_remain_valid_for_non_ascii_unit_symbols() {
        assert_eq!(
            render("20 °C in K"),
            "<Number>20</Number> <Unit>°C</Unit> <Operation>in</Operation> <Unit>K</Unit>"
        );
    }
}
