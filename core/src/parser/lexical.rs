use super::Span;
use chumsky::prelude::*;

pub(super) type Extra<'a> = extra::Err<Rich<'a, char>>;
pub(super) type Boxed<'a, O> = chumsky::Boxed<'a, 'a, &'a str, O, Extra<'a>>;

// Replace comment contents with spaces so spans keep their offsets.
pub(super) fn blank_comments(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = chars.clone();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if chars[i] == '/' && i + 1 < n && chars[i + 1] == '/' {
            while i < n {
                out[i] = ' ';
                i += 1;
            }
        } else if chars[i] == '/' && i + 1 < n && chars[i + 1] == '*' {
            out[i] = ' ';
            out[i + 1] = ' ';
            i += 2;
            while i < n {
                if chars[i] == '*' && i + 1 < n && chars[i + 1] == '/' {
                    out[i] = ' ';
                    out[i + 1] = ' ';
                    i += 2;
                    break;
                }
                out[i] = ' ';
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out.into_iter().collect()
}

/// Converts a chumsky span into a siffra [`Span`].
pub(super) fn to_span(span: SimpleSpan<usize>) -> Span {
    Span::new(span.start, span.end)
}

pub(super) fn word_boundary<'a>() -> Boxed<'a, ()> {
    any()
        .filter(|c: &char| !(c.is_ascii_alphanumeric() || *c == '_'))
        .rewind()
        .ignored()
        .or(end())
        .boxed()
}

pub(super) fn keyword<'a>(word: &'static str) -> Boxed<'a, &'a str> {
    just(word).then_ignore(word_boundary()).boxed()
}

// Shared numeric grammar for evaluation and syntax highlighting.
pub(super) fn number_parser<'a>() -> Boxed<'a, String> {
    let digit = any().filter(|c: &char| c.is_ascii_digit());
    let plain_digits: Boxed<'a, String> = digit
        .then(just('_').or_not().then(digit).repeated())
        .to_slice()
        .map(|s: &str| s.to_string())
        .boxed();

    let grouped_digits: Boxed<'a, String> = digit
        .repeated()
        .at_least(1)
        .at_most(3)
        .then(
            just(',')
                .ignore_then(digit.repeated().exactly(3))
                .repeated()
                .at_least(1),
        )
        .to_slice()
        .map(|s: &str| s.to_string())
        .boxed();

    let digits: Boxed<'a, String> = choice((grouped_digits, plain_digits)).boxed();

    let decimal: Boxed<'a, String> = choice((
        digits
            .clone()
            .then(just('.').ignore_then(digits.clone()).or_not())
            .to_slice()
            .map(|s: &str| s.to_string()),
        just('.')
            .ignore_then(digits.clone())
            .map(|frac| format!("0.{frac}")),
    ))
    .boxed();

    decimal
        .then(
            just('e')
                .or(just('E'))
                .ignore_then(just('-').or(just('+')).or_not())
                .then(digits)
                .or_not(),
        )
        .map(|(mut s, exp): (String, Option<(Option<char>, String)>)| {
            if let Some((sign, e)) = exp {
                s.push('E');
                if let Some(sign) = sign {
                    s.push(sign);
                }
                s.push_str(&e);
            }
            s
        })
        .boxed()
}

/// The chemical qualifier grammar shared by evaluation and tolerant syntax
/// analysis. Chemical validity is checked later by `Compound`.
pub(super) fn chemical_parser<'a>() -> Boxed<'a, String> {
    any()
        .filter(|c: &char| c.is_ascii_uppercase() || *c == '(')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '(' || *c == ')')
                .repeated(),
        )
        .to_slice()
        .map(|s: &str| s.to_string())
        .boxed()
}
