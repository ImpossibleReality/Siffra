pub mod representation;
mod span;
mod syntax;
mod temporal;

pub use representation::*;
pub use span::Span;
pub use syntax::{SyntaxKind, SyntaxSpan, syntax_spans};

use crate::error::SiffraExecutionError;
use crate::siffra_error;
use chumsky::error::{RichPattern, RichReason};
use chumsky::prelude::*;
use chumsky::text;

const RESERVED_VARS: &[&str] = &[
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
    "now",
    "today",
    "tomorrow",
    "yesterday",
];

/// The words that introduce a conversion target — or, when what follows is not
/// a dimension, the far endpoint of an interval.
const CONVERSION_KEYWORDS: &[&str] = &["into", "in", "as", "to"];

const KNOWN_FUNCS: &[&str] = &[
    "log", "ln", "sqrt", "sin", "cos", "tan", "asin", "acos", "atan", "root", "exp", "abs",
    "round", "floor", "ceil", "trunc", "sign", "sinh", "cosh", "tanh", "gamma",
];

const UNIT_EXCLUDE: &[&str] = &[
    "plus", "minus", "times", "of", "over", "by", "divided", "divide", "as", "to", "into", "on",
    "off", "what", "is",
];

// Replace comment contents with spaces so spans keep their offsets.
fn blank_comments(input: &str) -> String {
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

type Extra<'a> = extra::Err<Rich<'a, char>>;
type Boxed<'a, O> = chumsky::Boxed<'a, 'a, &'a str, O, Extra<'a>>;

fn is_known_unit(name: &str) -> bool {
    SCALAR_UNIT_ALIASES.contains(&name) || crate::representations::Unit::from_str(name).is_ok()
}

/// Whether `number unit number unit …` is one mixed-unit literal or an implicit product. It is
/// only parsed as mixed units when each unit measures the same thing and they get strictly smaller,
/// which is what makes `2 h 5 min` a duration while `2 m 5 kg` stays a product.
fn is_mixed_literal(rungs: &[(String, ParsedUnit)]) -> bool {
    let units = rungs
        .iter()
        .map(|(_, unit)| crate::representations::Unit::from_str(&unit.name).ok())
        .collect::<Option<Vec<_>>>();
    units.is_some_and(|units| crate::representations::ladder::validate(&units).is_ok())
}

/// What a `->` step asks for: one unit, or a ladder of them.
enum ArrowTarget {
    Single(ParsedDimension),
    Ladder(Vec<ParsedDimension>),
}

/// Converts a chumsky span into a siffra [`Span`].
fn to_span(span: SimpleSpan<usize>) -> Span {
    Span::new(span.start, span.end)
}

// Shared numeric grammar for evaluation and syntax highlighting.
fn number_parser<'a>() -> Boxed<'a, String> {
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
/// analysis. Chemical validity is checked later by `Compound`
fn chemical_parser<'a>() -> Boxed<'a, String> {
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

fn line_parser<'a>() -> impl Parser<'a, &'a str, ParsedLine, Extra<'a>> {
    let sp: Boxed<'a, ()> = just(' ').repeated().boxed();
    let sp1: Boxed<'a, ()> = just(' ').repeated().at_least(1).boxed();

    let word_boundary: Boxed<'a, ()> = any()
        .filter(|c: &char| !(c.is_ascii_alphanumeric() || *c == '_'))
        .rewind()
        .ignored()
        .or(end())
        .boxed();

    let kw = {
        let word_boundary = word_boundary.clone();
        move |w: &'static str| just(w).then_ignore(word_boundary.clone()).boxed()
    };

    let number_str = number_parser();

    let number_atom: Boxed<'a, ParsedExpr> = number_str
        .clone()
        .map_with(|value, e| ParsedExpr::Number {
            value,
            units: None,
            span: to_span(e.span()),
        })
        .boxed();

    let ascii_unit_name: Boxed<'a, String> = any()
        .filter(|c: &char| c.is_ascii_alphabetic())
        .repeated()
        .at_least(1)
        .collect::<String>()
        .boxed();
    let unit_name: Boxed<'a, String> = choice((
        just('°')
            .ignore_then(ascii_unit_name.clone())
            .map(|name| format!("°{name}")),
        ascii_unit_name,
    ))
    // Prevent a unit from being parsed as the prefix of an identifier
    .then_ignore(word_boundary.clone())
    .try_map(|s: String, span: SimpleSpan| {
        if UNIT_EXCLUDE.contains(&s.as_str()) {
            Err(Rich::custom(span, "operator word, not a unit"))
        } else {
            Ok(s)
        }
    })
    .boxed();

    let chemical_str = chemical_parser();
    let instant_literal = temporal::instant_literal();
    let clock_literal = temporal::clock_literal();
    let temporal_literal = temporal::temporal_literal();

    let unit_atom: Boxed<'a, ParsedUnit> = unit_name
        .clone()
        .then(sp1.clone().ignore_then(chemical_str.clone()).or_not())
        .map_with(|(name, chemical), e| ParsedUnit {
            name,
            chemical,
            span: to_span(e.span()),
        })
        .boxed();

    // A rung is a bare unit name: no chemical qualifier, no power, no product.
    let plain_unit: Boxed<'a, ParsedUnit> = unit_name
        .clone()
        .map_with(|name, e| ParsedUnit {
            name,
            chemical: None,
            span: to_span(e.span()),
        })
        .boxed();

    let mixed_rung: Boxed<'a, (String, ParsedUnit)> = number_str
        .clone()
        .then_ignore(sp1.clone())
        .then(plain_unit.clone())
        .boxed();

    // `5 ft 2 in` is one length. The rungs have to agree on what they measure
    // and shrink left to right, or the line keeps today's reading.
    let mixed_literal: Boxed<'a, ParsedExpr> = mixed_rung
        .clone()
        .then(
            sp1.clone()
                .ignore_then(mixed_rung)
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .try_map(
            |(first, rest): ((String, ParsedUnit), Vec<(String, ParsedUnit)>), span: SimpleSpan| {
                let mut rungs = vec![first];
                rungs.extend(rest);
                if is_mixed_literal(&rungs) {
                    Ok(ParsedExpr::Mixed {
                        rungs,
                        span: to_span(span),
                    })
                } else {
                    Err(Rich::custom(span, "not a mixed-unit literal"))
                }
            },
        )
        .boxed();

    // Feet and inches shorthand (e.x. 5'2")
    let quoted_length: Boxed<'a, ParsedExpr> = number_str
        .clone()
        .then_ignore(just('\''))
        .then(number_str.clone())
        .then_ignore(just('"'))
        .map_with(|(feet, inches): (String, String), e| {
            let span = to_span(e.span());
            let unit = |name: &str| ParsedUnit {
                name: name.to_string(),
                chemical: None,
                span,
            };
            ParsedExpr::Mixed {
                rungs: vec![(feet, unit("ft")), (inches, unit("in"))],
                span,
            }
        })
        .boxed();

    let unit_power: Boxed<'a, String> = just('-')
        .or_not()
        .then(number_str.clone())
        .map(|(neg, mut number): (Option<char>, String)| {
            if neg.is_some() {
                number.insert(0, '-');
            }
            number
        })
        .boxed();

    let bare_unit_pow: Boxed<'a, (ParsedUnit, String)> = unit_atom
        .clone()
        .then(just('^').ignore_then(unit_power.clone()).or_not())
        .map(|(unit, pow)| (unit, pow.unwrap_or_else(|| "1".to_string())))
        .boxed();

    let bare_unit_mul_group: Boxed<'a, Vec<(ParsedUnit, String)>> = bare_unit_pow
        .clone()
        .then(
            just('*')
                .ignore_then(bare_unit_pow.clone())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| {
            let mut v = vec![first];
            v.extend(rest);
            v
        })
        .boxed();

    // Bare units are compact: no whitespace around `*`, `/`, or `^`. A leading
    // `1/` is accepted only for denominator-only dimensions emitted by Display.
    // This keeps `as rad * 2` from claiming `* 2` as part of the target.
    let ordinary_bare_units = bare_unit_mul_group
        .clone()
        .then(just('/').ignore_then(bare_unit_mul_group.clone()).or_not())
        .map(|(numerator, denominator)| (numerator, denominator.unwrap_or_default()))
        .boxed();
    let inverse_bare_units = just('1')
        .then_ignore(just('/'))
        .ignore_then(bare_unit_mul_group.clone())
        .map(|denominator| (Vec::new(), denominator))
        .boxed();
    let bare_units: Boxed<'a, ParsedDimension> = choice((inverse_bare_units, ordinary_bare_units))
        .map_with(|(numerator, denominator), e| ParsedDimension {
            numerator,
            denominator,
            span: to_span(e.span()),
        })
        .boxed();

    let bracketed_unit_pow: Boxed<'a, (ParsedUnit, String)> = unit_atom
        .then(
            just('^')
                .padded_by(sp.clone())
                .ignore_then(unit_power)
                .or_not(),
        )
        .map(|(unit, pow)| (unit, pow.unwrap_or_else(|| "1".to_string())))
        .boxed();

    let bracketed_unit_mul_group = bracketed_unit_pow
        .clone()
        .then(
            just('*')
                .padded_by(sp.clone())
                .ignore_then(bracketed_unit_pow.clone())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| {
            let mut v = vec![first];
            v.extend(rest);
            v
        })
        .boxed();

    let bracketed_unit_group: Boxed<'a, Vec<(ParsedUnit, String)>> = choice((
        bracketed_unit_mul_group.clone(),
        just('1').map(|_| Vec::<(ParsedUnit, String)>::new()),
    ))
    .boxed();

    let bracketed_units: Boxed<'a, ParsedDimension> = bracketed_unit_group
        .clone()
        .then(
            just('/')
                .padded_by(sp.clone())
                .ignore_then(bracketed_unit_group.clone())
                .or_not(),
        )
        .map_with(|(numerator, denominator), e| ParsedDimension {
            numerator,
            denominator: denominator.unwrap_or_default(),
            span: to_span(e.span()),
        })
        .padded_by(sp.clone())
        .delimited_by(just('['), just(']'))
        .boxed();

    // Symbolic scalar units need an explicit branch because ordinary unit
    // names are alphabetic. `%` is scaled by 1/100; `#` is unscaled/unitless.
    let symbolic_units: Boxed<'a, ParsedDimension> =
        choice((just('%').to("percent"), just('#').to("#")))
            .map_with(|name: &str, e| {
                let span = to_span(e.span());
                ParsedDimension {
                    numerator: vec![(
                        ParsedUnit {
                            name: name.to_string(),
                            chemical: None,
                            span,
                        },
                        "1".to_string(),
                    )],
                    denominator: Vec::new(),
                    span,
                }
            })
            .boxed();

    let units: Boxed<'a, ParsedDimension> = choice((
        symbolic_units,
        bracketed_units,
        temporal::zone_dimension(),
        bare_units,
    ))
    .boxed();

    let dimensional_number: Boxed<'a, ParsedExpr> = number_str
        .clone()
        .then(sp1.clone().ignore_then(units.clone()))
        .map_with(|(value, units), e| ParsedExpr::Number {
            value,
            units: Some(units),
            span: to_span(e.span()),
        })
        .boxed();

    // Currency symbols are prefix notation, unlike ordinary units. `$` is
    // unambiguously USD; ISO codes such as `EUR` continue to use `40 EUR`.
    let dollar_number: Boxed<'a, ParsedExpr> = just('$')
        .ignore_then(number_str.clone())
        .map_with(|value, e| {
            let span = to_span(e.span());
            ParsedExpr::Number {
                value,
                units: Some(ParsedDimension {
                    numerator: vec![(
                        ParsedUnit {
                            name: "USD".to_string(),
                            chemical: None,
                            span,
                        },
                        "1".to_string(),
                    )],
                    denominator: Vec::new(),
                    span,
                }),
                span,
            }
        })
        .boxed();

    let conversion_keyword: Boxed<'a, ()> = sp1
        .clone()
        .ignore_then(choice(
            CONVERSION_KEYWORDS
                .iter()
                .map(|word| kw(word))
                .collect::<Vec<_>>(),
        ))
        .ignore_then(sp1.clone())
        .ignored()
        .boxed();

    // A conversion target has to name units that exist. An unknown name is not
    // a failed conversion but a different reading of the line, so it falls
    // through to the interval endpoint that `launch to arrival` needs.
    let known_units: Boxed<'a, ParsedDimension> = units
        .clone()
        .try_map(|dimension: ParsedDimension, span: SimpleSpan| {
            if dimension
                .numerator
                .iter()
                .chain(&dimension.denominator)
                .all(|(unit, _)| is_known_unit(&unit.name))
            {
                Ok(dimension)
            } else {
                Err(Rich::custom(span, "not a unit"))
            }
        })
        .boxed();

    let convert: Boxed<'a, ParsedDimension> =
        conversion_keyword.clone().ignore_then(known_units).boxed();

    let rung: Boxed<'a, ParsedDimension> = plain_unit
        .clone()
        .try_map(|unit: ParsedUnit, span: SimpleSpan| {
            if is_known_unit(&unit.name) {
                Ok(ParsedDimension {
                    span: unit.span,
                    numerator: vec![(unit, "1".to_string())],
                    denominator: Vec::new(),
                })
            } else {
                Err(Rich::custom(span, "not a unit"))
            }
        })
        .boxed();

    let bare_ladder: Boxed<'a, Vec<ParsedDimension>> = rung
        .clone()
        .then(
            just(',')
                .ignore_then(rung.clone())
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest): (ParsedDimension, Vec<ParsedDimension>)| {
            let mut rungs = vec![first];
            rungs.extend(rest);
            rungs
        })
        .boxed();

    let bracketed_ladder: Boxed<'a, Vec<ParsedDimension>> = rung
        .clone()
        .separated_by(just(',').padded_by(sp.clone()))
        .at_least(2)
        .collect::<Vec<_>>()
        .padded_by(sp.clone())
        .delimited_by(just('['), just(']'))
        .boxed();

    let ladder_target: Boxed<'a, Vec<ParsedDimension>> =
        choice((bracketed_ladder, bare_ladder)).boxed();

    let convert_ladder: Boxed<'a, Vec<ParsedDimension>> = conversion_keyword
        .clone()
        .ignore_then(ladder_target.clone())
        .boxed();

    let variable: Boxed<'a, ParsedExpr> = text::ascii::ident()
        .try_map(|name: &str, span: SimpleSpan| {
            if RESERVED_VARS.contains(&name) {
                Err(Rich::custom(span, "reserved word"))
            } else {
                Ok(ParsedExpr::Variable {
                    name: name.to_string(),
                    span: to_span(span),
                })
            }
        })
        .boxed();

    // Function names are alphabetic, except for the conventional `atan2` name.
    // Other trailing numbers keep the base shorthand (`log2(x)` -> `log_2(x)`).
    let func_name: Boxed<'a, (String, Span)> = choice((
        just("atan2").map_with(|name: &str, e| (name.to_string(), to_span(e.span()))),
        any()
            .filter(|c: &char| c.is_ascii_alphabetic())
            .repeated()
            .at_least(1)
            .collect::<String>()
            .map_with(|name, e| (name, to_span(e.span()))),
    ))
    .boxed();

    let mul_op: Boxed<'a, (Op, Span)> = {
        let kw = kw.clone();
        let sp = sp.clone();
        let sp1 = sp1.clone();
        let percent_token = choice((just('%').ignored(), kw("percent").ignored())).boxed();

        let relative_percent = sp1
            .clone()
            .ignore_then(kw("as"))
            .then_ignore(sp1.clone())
            .then_ignore(kw("a"))
            .then_ignore(sp1.clone())
            .then_ignore(percent_token)
            .then_ignore(sp1.clone())
            .ignore_then(choice((
                kw("of").to(Op::PercentageRelativeOf),
                kw("on").to(Op::PercentageRelativeOn),
                kw("off").to(Op::PercentageRelativeOff),
            )))
            .boxed();

        let inverse_percent = sp1
            .clone()
            .ignore_then(choice((
                kw("of").to(Op::PercentageOfWhat),
                kw("on").to(Op::PercentageOnWhat),
                kw("off").to(Op::PercentageOffWhat),
            )))
            .then_ignore(sp1.clone())
            .then_ignore(kw("what"))
            .then_ignore(sp1.clone())
            .then_ignore(kw("is"))
            .boxed();

        choice((
            relative_percent,
            inverse_percent,
            sp.clone().ignore_then(just('*')).to(Op::Multiply),
            sp.clone().ignore_then(just('/')).to(Op::Divide),
            sp1.clone().ignore_then(kw("times")).to(Op::Multiply),
            sp1.clone().ignore_then(kw("of")).to(Op::Multiply),
            sp1.clone().ignore_then(kw("on")).to(Op::PercentageOn),
            sp1.clone().ignore_then(kw("off")).to(Op::PercentageOff),
            sp1.clone().ignore_then(kw("over")).to(Op::Divide),
            sp1.clone().ignore_then(kw("divided by")).to(Op::Divide),
            sp1.clone().ignore_then(kw("divide by")).to(Op::Divide),
            sp1.clone().ignore_then(kw("by")).to(Op::Divide),
        ))
        .then_ignore(sp.clone())
        .map_with(|op, e| (op, to_span(e.span())))
        .boxed()
    };

    let add_op: Boxed<'a, (Op, Span)> = {
        let kw = kw.clone();
        let sp = sp.clone();
        let sp1 = sp1.clone();
        choice((
            sp.clone().ignore_then(just('+')).to(Op::Add),
            sp.clone().ignore_then(just('-')).to(Op::Subtract),
            sp1.clone().ignore_then(kw("plus")).to(Op::Add),
            sp1.clone().ignore_then(kw("minus")).to(Op::Subtract),
        ))
        .then_ignore(sp.clone())
        .map_with(|op, e| (op, to_span(e.span())))
        .boxed()
    };

    let expr = recursive(|expr| {
        let paren_expr = just('(')
            .ignore_then(expr.clone().padded_by(sp.clone()))
            .then_ignore(just(')'))
            .boxed();

        let args = expr
            .clone()
            .padded_by(sp.clone())
            .separated_by(just(','))
            .at_least(1)
            .collect::<Vec<_>>()
            .delimited_by(just('('), just(')'))
            .boxed();

        let func_base = choice((
            just('_').ignore_then(choice((
                number_atom.clone(),
                paren_expr.clone(),
                variable.clone(),
            ))),
            number_atom.clone(),
        ))
        .boxed();

        let function = func_name
            .clone()
            .then(func_base.or_not())
            .then(args)
            .map_with(
                |(((name, function_span), base), args), e| ParsedExpr::FunctionCall {
                    name,
                    args,
                    base: base.map(Box::new),
                    span: to_span(e.span()),
                    function_span,
                },
            )
            .boxed();

        // Implicit multiplication binds below exponentiation.
        let product = recursive(|product| {
            let ungrouped_function = func_name
                .clone()
                .then_ignore(sp1.clone())
                .then(product.clone())
                .try_map(
                    |((name, function_span), arg): ((String, Span), ParsedExpr),
                     span: SimpleSpan| {
                        if KNOWN_FUNCS.contains(&name.as_str()) {
                            Ok(ParsedExpr::FunctionCall {
                                name,
                                args: vec![arg],
                                base: None,
                                span: to_span(span),
                                function_span,
                            })
                        } else {
                            Err(Rich::custom(span, "not a known function"))
                        }
                    },
                )
                .boxed();

            let atom = choice((
                paren_expr.clone(),
                function.clone(),
                // Digit-led temporal literals must win over the arithmetic
                // reading of the same characters.
                instant_literal.clone(),
                clock_literal.clone(),
                quoted_length.clone(),
                // A mixed literal has to be tried before the single-unit
                // number it starts with, which would otherwise take the first
                // rung and leave the rest to implicit multiplication.
                mixed_literal.clone(),
                dollar_number.clone(),
                dimensional_number.clone(),
                ungrouped_function,
                number_atom.clone(),
                variable.clone(),
            ))
            .boxed();

            let factorial = atom
                .then(
                    sp.clone()
                        .ignore_then(just('!'))
                        .map_with(|_, e| to_span(e.span()))
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .map(|(mut e, bangs): (ParsedExpr, Vec<Span>)| {
                    for span in bangs {
                        e = ParsedExpr::UnOpPost {
                            lhs: Box::new(e),
                            op: OpPost::Factorial,
                            span,
                        };
                    }
                    e
                })
                .boxed();

            let unary = recursive(|unary| {
                let power = factorial
                    .clone()
                    .then(
                        just('^')
                            .padded_by(sp.clone())
                            .ignore_then(unary.clone())
                            .or_not(),
                    )
                    .map_with(|(base, exp), e| match exp {
                        Some(exponent) => ParsedExpr::BinOp {
                            lhs: Box::new(base),
                            op: Op::Exponent,
                            rhs: Box::new(exponent),
                            span: to_span(e.span()),
                        },
                        None => base,
                    })
                    .boxed();

                let percent = power
                    .then(
                        sp.clone()
                            .ignore_then(just('%'))
                            .map_with(|_, e| to_span(e.span()))
                            .repeated()
                            .collect::<Vec<_>>(),
                    )
                    .map(|(mut e, percents): (ParsedExpr, Vec<Span>)| {
                        for span in percents {
                            e = ParsedExpr::UnOpPost {
                                lhs: Box::new(e),
                                op: OpPost::Percent,
                                span,
                            };
                        }
                        e
                    })
                    .boxed();

                sp.clone()
                    .ignore_then(just('-'))
                    .then_ignore(sp.clone())
                    .map_with(|_, e| to_span(e.span()))
                    .repeated()
                    .collect::<Vec<_>>()
                    .then(percent)
                    .map(|(negs, mut e): (Vec<Span>, ParsedExpr)| {
                        for span in negs.into_iter().rev() {
                            e = ParsedExpr::UnOpPre {
                                op: OpPre::Negate,
                                rhs: Box::new(e),
                                span,
                            };
                        }
                        e
                    })
            })
            .boxed();

            let nonnegative_unary = sp
                .clone()
                .ignore_then(just('-'))
                .not()
                .rewind()
                .ignore_then(unary.clone());
            unary
                .clone()
                .then(nonnegative_unary.repeated().collect::<Vec<_>>())
                .map_with(|(first, rest): (ParsedExpr, Vec<ParsedExpr>), e| {
                    let span = to_span(e.span());
                    let mut acc = first;
                    for r in rest {
                        acc = ParsedExpr::BinOp {
                            lhs: Box::new(acc),
                            op: Op::Multiply,
                            rhs: Box::new(r),
                            span,
                        };
                    }
                    acc
                })
                .boxed()
        });

        enum MulTail {
            Arithmetic(Op, Span, ParsedExpr),
            Convert(ParsedDimension, Span),
            Ladder(Vec<ParsedDimension>, Span),
            Interval(ParsedExpr, Span),
        }

        // `to` and its synonyms are one operator with two readings: a unit to
        // convert into, or the far endpoint of an interval. A written-out date
        // is tried first because month names such as `dec` are also units;
        // anything else a dimension cannot claim falls through to an endpoint.
        let interval_endpoint = conversion_keyword
            .clone()
            .ignore_then(temporal_literal.clone())
            .boxed();
        let interval_expression = conversion_keyword
            .clone()
            .ignore_then(product.clone())
            .boxed();

        // Conversion is a postfix at multiplication precedence.
        let mul_tail = choice((
            mul_op
                .clone()
                .then(product.clone())
                .map(|((op, span), rhs)| MulTail::Arithmetic(op, span, rhs)),
            interval_endpoint.map_with(|rhs, e| MulTail::Interval(rhs, to_span(e.span()))),
            // A ladder target has to be tried first: a single-unit target would
            // claim `ft` and leave `,in` behind.
            convert_ladder
                .clone()
                .map_with(|rungs, e| MulTail::Ladder(rungs, to_span(e.span()))),
            convert
                .clone()
                .map_with(|dim, e| MulTail::Convert(dim, to_span(e.span()))),
            interval_expression.map_with(|rhs, e| MulTail::Interval(rhs, to_span(e.span()))),
        ))
        .boxed();

        let mul = product
            .then(mul_tail.repeated().collect::<Vec<_>>())
            .map(|(first, rest): (ParsedExpr, Vec<MulTail>)| {
                let mut lhs = first;
                let mut tails = rest.into_iter().peekable();

                while let Some(tail) = tails.next() {
                    let is_percentage_application = matches!(
                        tail,
                        MulTail::Arithmetic(Op::PercentageOn | Op::PercentageOff, ..)
                    );

                    if is_percentage_application {
                        let MulTail::Arithmetic(first_op, first_span, first_rhs) = tail else {
                            unreachable!("percentage applications are arithmetic tails")
                        };
                        let mut chain = Vec::new();

                        while matches!(
                            tails.peek(),
                            Some(MulTail::Arithmetic(
                                Op::PercentageOn | Op::PercentageOff,
                                ..
                            ))
                        ) {
                            let Some(MulTail::Arithmetic(op, span, rhs)) = tails.next() else {
                                unreachable!("peeked percentage application must be present")
                            };
                            chain.push((op, span, rhs));
                        }

                        let rhs = if let Some((mut op, mut span, mut rhs)) = chain.pop() {
                            while let Some((previous_op, previous_span, previous_rhs)) = chain.pop()
                            {
                                rhs = ParsedExpr::BinOp {
                                    lhs: Box::new(previous_rhs),
                                    op,
                                    rhs: Box::new(rhs),
                                    span,
                                };
                                op = previous_op;
                                span = previous_span;
                            }

                            ParsedExpr::BinOp {
                                lhs: Box::new(first_rhs),
                                op,
                                rhs: Box::new(rhs),
                                span,
                            }
                        } else {
                            first_rhs
                        };

                        lhs = ParsedExpr::BinOp {
                            lhs: Box::new(lhs),
                            op: first_op,
                            rhs: Box::new(rhs),
                            span: first_span,
                        };
                        continue;
                    }

                    lhs = match tail {
                        MulTail::Arithmetic(op, span, rhs) => ParsedExpr::BinOp {
                            lhs: Box::new(lhs),
                            op,
                            rhs: Box::new(rhs),
                            span,
                        },
                        MulTail::Convert(dim, span) => ParsedExpr::UnOpPost {
                            lhs: Box::new(lhs),
                            op: OpPost::Convert(dim),
                            span,
                        },
                        MulTail::Ladder(rungs, span) => ParsedExpr::UnOpPost {
                            lhs: Box::new(lhs),
                            op: OpPost::Ladder(rungs),
                            span,
                        },
                        MulTail::Interval(rhs, span) => ParsedExpr::BinOp {
                            lhs: Box::new(lhs),
                            op: Op::Interval,
                            rhs: Box::new(rhs),
                            span,
                        },
                    };
                }
                lhs
            })
            .boxed();

        mul.clone()
            .then(add_op.clone().then(mul).repeated().collect::<Vec<_>>())
            .map(|(first, rest)| {
                let mut lhs: ParsedExpr = first;
                for ((op, span), rhs) in rest {
                    lhs = ParsedExpr::BinOp {
                        lhs: Box::new(lhs),
                        op,
                        rhs: Box::new(rhs),
                        span,
                    };
                }
                lhs
            })
    });

    let ident_lhs: Boxed<'a, String> = text::ascii::ident()
        .map(|name: &str| name.to_string())
        .boxed();

    let rest_of_line: Boxed<'a, String> = any().repeated().collect::<String>().boxed();

    // `->` converts the complete line and must be last. A chain of them reads
    // left to right, which is how a span is asked for one reading after another.
    let arrow_step = sp
        .clone()
        .ignore_then(just("->").map_with(|_, e| to_span(e.span())))
        .then_ignore(sp.clone())
        .then(choice((
            ladder_target.map(ArrowTarget::Ladder),
            units.clone().map(ArrowTarget::Single),
        )))
        .boxed();

    let arrow_conversion = arrow_step
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .then_ignore(sp.clone())
        .then(rest_of_line)
        .validate(
            |(steps, rest): (Vec<(Span, ArrowTarget)>, String), _, emitter| {
                if !rest.is_empty() {
                    let arrow_span = steps.last().expect("at least one arrow").0;
                    emitter.emit(Rich::custom(
                        SimpleSpan::from(arrow_span.start()..arrow_span.end()),
                        "`-> [Unit]` must be at the end of a line",
                    ));
                }
                steps
            },
        )
        .or_not()
        .boxed();

    let line_expr: Boxed<'a, ParsedExpr> = expr
        .clone()
        .then(arrow_conversion)
        .map(|(e, conversions)| {
            let mut lhs = e;
            for (arrow_span, target) in conversions.into_iter().flatten() {
                let (target_span, op) = match target {
                    ArrowTarget::Single(dim) => (dim.span, OpPost::Convert(dim)),
                    ArrowTarget::Ladder(rungs) => {
                        let span = rungs
                            .iter()
                            .map(|rung| rung.span)
                            .reduce(|left, right| left.merge(right))
                            .expect("a ladder has rungs");
                        (span, OpPost::Ladder(rungs))
                    }
                };
                lhs = ParsedExpr::UnOpPost {
                    lhs: Box::new(lhs),
                    op,
                    span: arrow_span.merge(target_span),
                };
            }
            lhs
        })
        .boxed();

    let let_assign = just("let")
        .then_ignore(word_boundary.clone())
        .ignore_then(sp1.clone())
        .ignore_then(ident_lhs.clone())
        .then_ignore(sp.clone())
        .then_ignore(just('='))
        .then(sp.clone().ignore_then(line_expr.clone()))
        .map(|(name, e)| ParsedLine::Variable(name, e))
        .boxed();

    let var_assign = ident_lhs
        .then_ignore(sp.clone())
        .then_ignore(just('='))
        .then(sp.clone().ignore_then(line_expr.clone()))
        .map(|(name, e)| ParsedLine::Variable(name, e))
        .boxed();

    sp.clone()
        .ignore_then(choice((
            let_assign,
            var_assign,
            line_expr.map(ParsedLine::Expression),
        )))
        .then_ignore(sp.clone())
        .then_ignore(end())
}

/// Parses a `#`-prefixed header line.
pub fn parse_header(input: &str) -> Option<String> {
    let rest = input.trim_start().strip_prefix('#')?;
    Some(rest.trim_start_matches('#').trim().to_string())
}

/// Parses one line into an expression, assignment, header, or comment.
pub fn parse_line(input: &str) -> Result<ParsedLine, SiffraExecutionError> {
    if let Some(title) = parse_header(input) {
        return Ok(ParsedLine::Header(title));
    }

    let cleaned = blank_comments(input);
    if cleaned.trim().is_empty() {
        return Ok(ParsedLine::Comment);
    }

    line_parser()
        .parse(cleaned.as_str())
        .into_result()
        .map_err(|errs| {
            let err = errs
                .into_iter()
                .max_by_key(|e| e.span().end)
                .expect("parse failure always yields at least one error");
            let span = *err.span();
            let (message, description) = describe_error(&err, &cleaned);
            siffra_error!(message, description, Some(Span::new(span.start, span.end)))
        })
}

fn describe_error(err: &Rich<'_, char>, input: &str) -> (String, String) {
    if is_incomplete_assignment(input) {
        return (
            "Incomplete expression".to_string(),
            "Expected an expression after '='".to_string(),
        );
    }

    match err.reason() {
        RichReason::Custom(msg) => ("Syntax Error".to_string(), msg.clone()),
        RichReason::ExpectedFound { expected, .. } => match err.found() {
            Some(c) => (
                format!("Unexpected '{}'", c),
                format!("The character '{}' isn't valid here", c),
            ),
            None => {
                if let Some(closing) = expected.iter().find_map(closing_delimiter) {
                    return (
                        "Unclosed bracket".to_string(),
                        format!("Missing a closing '{}'", closing),
                    );
                }

                let trimmed = input.trim_end();
                let tail = trimmed.chars().last();
                match tail {
                    Some(op) if "+-*/^".contains(op) => (
                        "Incomplete expression".to_string(),
                        format!("Expected something after '{}'", op),
                    ),
                    _ => (
                        "Incomplete expression".to_string(),
                        "The line ended unexpectedly".to_string(),
                    ),
                }
            }
        },
    }
}

fn closing_delimiter(pattern: &RichPattern<'_, char>) -> Option<char> {
    match pattern {
        RichPattern::Token(token) => matches!(**token, ')' | ']' | '}').then(|| **token),
        _ => None,
    }
}

fn is_incomplete_assignment(input: &str) -> bool {
    let Some(lhs) = input.trim_end().strip_suffix('=') else {
        return false;
    };
    let lhs = lhs.trim();
    let name = match lhs.strip_prefix("let") {
        Some(rest) if rest.starts_with(char::is_whitespace) => rest.trim(),
        _ => lhs,
    };

    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> ParsedLine {
        parse_line(s).unwrap_or_else(|e| panic!("failed to parse {:?}: {:?}", s, e.message()))
    }

    #[test]
    fn syntax_errors_are_described_by_kind() {
        let describe = |input: &str| {
            let error = parse_line(input).expect_err("line should fail");
            (error.message(), error.description().expect("description"))
        };

        assert_eq!(
            describe("(5 + 2"),
            (
                "Unclosed bracket".to_string(),
                "Missing a closing ')'".to_string()
            )
        );
        assert_eq!(
            describe("5 *"),
            (
                "Incomplete expression".to_string(),
                "Expected something after '*'".to_string()
            )
        );
        assert_eq!(
            describe("@#$"),
            (
                "Unexpected '@'".to_string(),
                "The character '@' isn't valid here".to_string()
            )
        );
    }

    #[test]
    fn parses_simple_expression() {
        assert!(matches!(parse("5 + 2 / 4 - 2"), ParsedLine::Expression(_)));
    }

    #[test]
    fn parses_variable_assignment() {
        assert!(matches!(parse("x = 5"), ParsedLine::Variable(name, _) if name == "x"));
        assert!(matches!(parse("let y = 5"), ParsedLine::Variable(name, _) if name == "y"));
    }

    #[test]
    fn parses_comments() {
        assert!(matches!(parse("// a comment"), ParsedLine::Comment));
        assert!(matches!(parse("/* block */"), ParsedLine::Comment));
        assert!(matches!(parse("   "), ParsedLine::Comment));
    }

    #[test]
    fn parses_headers() {
        assert!(matches!(parse("# Trip budget"), ParsedLine::Header(t) if t == "Trip budget"));
        assert!(matches!(parse("  ## Notes  "), ParsedLine::Header(t) if t == "Notes"));
        assert!(matches!(parse("#"), ParsedLine::Header(t) if t.is_empty()));
        assert!(matches!(parse("5 #"), ParsedLine::Expression(_)));
    }

    #[test]
    fn parses_dimensional_numbers() {
        assert!(matches!(parse("5 m + 3 in"), ParsedLine::Expression(_)));
        assert!(matches!(
            parse("accel = 5 m/s^2"),
            ParsedLine::Variable(_, _)
        ));
    }

    #[test]
    fn compound_units_allow_brackets_and_displayed_inverse_units() {
        for input in [
            "5 [kg * m / s^2]",
            "5 [1/s]",
            "5 [s/1]",
            "5 [ 1 / s ]",
            "5 1/s",
            "5 s in 1/s",
            "5 g O2 in mol O2",
            "5 mol C6H12O6 in g",
            "18 [g H2O] as [mol H2O]",
        ] {
            parse(input);
        }
    }

    #[test]
    fn parses_functions() {
        parse("sin(cos(5))");
        parse("log2(100)");
        parse("log_(2+3)(100)");
        parse("5 ^ log_(2+3)(100)");
        parse("log 100");
        parse("sqrt 2");
    }

    #[test]
    fn parses_natural_language() {
        parse("5 times 20 plus 2");
        parse("10% of 20");
        parse("100 divided by 4");
    }

    #[test]
    fn chained_percentage_applications_are_right_associative() {
        let ParsedLine::Expression(ParsedExpr::BinOp {
            lhs,
            op: Op::PercentageOn,
            rhs,
            ..
        }) = parse("5% on 2% on $5")
        else {
            panic!("outer percentage application should be parsed");
        };

        assert!(matches!(
            *lhs,
            ParsedExpr::UnOpPost {
                op: OpPost::Percent,
                ..
            }
        ));
        assert!(matches!(
            *rhs,
            ParsedExpr::BinOp {
                op: Op::PercentageOn,
                ..
            }
        ));
    }

    #[test]
    fn parses_conversions() {
        parse("(5 m) in km");
        parse("5 m in km");
        parse("2 s as ms");
    }

    #[test]
    fn conversion_binds_looser_than_multiplication_and_division() {
        for input in ["100 kg * 9.8 m/s^2 in N", "10 W * 5 s as J"] {
            let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) = parse(input) else {
                panic!("conversion should be the outer expression for {input:?}");
            };

            assert!(matches!(op, OpPost::Convert(_)));
            assert!(matches!(
                *lhs,
                ParsedExpr::BinOp {
                    op: Op::Multiply,
                    ..
                }
            ));
        }

        let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) =
            parse("100 km / 2 h in mph")
        else {
            panic!("conversion should be the outer expression for a quotient");
        };

        assert!(matches!(op, OpPost::Convert(_)));
        assert!(matches!(*lhs, ParsedExpr::BinOp { op: Op::Divide, .. }));
    }

    #[test]
    fn arithmetic_can_follow_a_conversion() {
        let ParsedLine::Expression(ParsedExpr::BinOp {
            lhs,
            op: Op::Multiply,
            ..
        }) = parse("190 deg as rad  *2")
        else {
            panic!("multiplication should follow the completed conversion");
        };

        assert!(matches!(
            *lhs,
            ParsedExpr::UnOpPost {
                op: OpPost::Convert(_),
                ..
            }
        ));
    }

    #[test]
    fn end_of_line_arrow_converts_the_whole_expression() {
        let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) =
            parse("5 m + 3 in -> cm")
        else {
            panic!("`->` should wrap the whole line expression");
        };
        assert!(matches!(op, OpPost::Convert(_)));
        assert!(matches!(*lhs, ParsedExpr::BinOp { op: Op::Add, .. }));

        parse("5 m + 3 in ->cm");
        let ParsedLine::Variable(name, ParsedExpr::UnOpPost { op, .. }) =
            parse("x = 5 m + 3 in -> cm")
        else {
            panic!("`->` on an assignment should convert the stored value");
        };
        assert_eq!(name, "x");
        assert!(matches!(op, OpPost::Convert(_)));
    }

    #[test]
    fn arrow_is_only_valid_at_the_end_of_the_line() {
        assert!(parse_line("(5 -> cm) + 2").is_err());

        for input in ["5 m -> cm + 2", "5 m -> cm * 3", "5 m -> cm in mm"] {
            let error = parse_line(input).expect_err("content after `-> unit` is invalid");
            assert_eq!(
                error.description().as_deref(),
                Some("`-> [Unit]` must be at the end of a line")
            );
            let arrow = input.find("->").unwrap();
            assert_eq!(error.span(), Some((arrow, arrow + 2)));
        }
    }

    #[test]
    fn parses_temporal_literals() {
        for input in [
            "2024-02-10",
            "2024-02-10 12:00",
            "2024-02-10 12:00:30",
            "2024-02-10 12:00 America/Los_Angeles",
            "2024-02-10 12:00 UTC",
            "2024-02-10 12:00 +05:30",
            "5pm UTC on feb 2 2026",
            "5pm UTC today",
            "5pm UTC",
            "5pm today",
            "now",
            "today + 1 day",
            "9:00",
            "17:00 - 9:15",
        ] {
            parse(input);
        }
    }

    #[test]
    fn a_temporal_literal_after_a_conversion_keyword_makes_a_range() {
        let ParsedLine::Expression(ParsedExpr::BinOp {
            op: Op::Interval,
            lhs,
            rhs,
            ..
        }) = parse("2024-02-10 to 2024-03-10")
        else {
            panic!("a range should lower to an interval");
        };

        let month = |expr: &ParsedExpr| match expr {
            ParsedExpr::Instant { instant, .. } => instant.date.expect("a dated literal").month,
            other => panic!("expected an instant, got {other:?}"),
        };
        assert_eq!(month(&lhs), 2);
        assert_eq!(month(&rhs), 3);
    }

    #[test]
    fn a_range_endpoint_may_be_any_expression_a_unit_cannot_claim() {
        for input in ["now to launch", "9:00 to (8:00 + 3 h)", "today to tomorrow"] {
            let ParsedLine::Expression(ParsedExpr::BinOp {
                op: Op::Interval, ..
            }) = parse(input)
            else {
                panic!("{input:?} should lower to an interval");
            };
        }
    }

    #[test]
    fn a_dimension_after_a_conversion_keyword_is_still_a_conversion() {
        for input in [
            "2024-02-10 to 2024-03-10 in days",
            "5 m in km",
            "3 in in cm",
        ] {
            let ParsedLine::Expression(expression) = parse(input) else {
                panic!("{input:?} should be an expression");
            };
            assert!(
                matches!(
                    expression,
                    ParsedExpr::UnOpPost {
                        op: OpPost::Convert(_),
                        ..
                    }
                ),
                "{input:?} should end in a conversion"
            );
        }
    }

    #[test]
    fn zone_names_never_claim_a_compound_unit() {
        parse("5 m in m/s");
        parse("100 km / 2 h in mph");
        assert!(parse_line("2024-02-10 12:00 Bogus/Zone").is_err());
    }

    #[test]
    fn arrow_conversions_chain_but_stay_at_the_end_of_the_line() {
        let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) =
            parse("2024-02-10 to 2024-03-10 -> day -> hour")
        else {
            panic!("the last arrow should be the outer conversion");
        };
        assert!(matches!(op, OpPost::Convert(_)));
        assert!(matches!(
            *lhs,
            ParsedExpr::UnOpPost {
                op: OpPost::Convert(_),
                ..
            }
        ));

        assert!(parse_line("5 m -> cm -> mm + 2").is_err());
    }

    #[test]
    fn a_shrinking_run_of_one_dimension_is_one_mixed_literal() {
        for (input, rungs) in [
            ("5 ft 2 in", 2),
            ("2 h 5 min 2 s", 3),
            ("6 lb 3 oz", 2),
            ("5'2\"", 2),
        ] {
            let ParsedLine::Expression(ParsedExpr::Mixed { rungs: parsed, .. }) = parse(input)
            else {
                panic!("{input:?} should be one mixed literal");
            };
            assert_eq!(parsed.len(), rungs, "input was {input:?}");
        }

        for input in ["2 m 5 kg", "5 m 2 m", "2 in 5 ft", "5 ft 2"] {
            assert!(
                !matches!(
                    parse_line(input),
                    Ok(ParsedLine::Expression(ParsedExpr::Mixed { .. }))
                ),
                "{input:?} should not merge"
            );
        }
    }

    #[test]
    fn commas_and_brackets_mark_a_ladder_target_and_nothing_else_does() {
        for input in [
            "1.87 m to ft,in",
            "1.87 m to [ft, in]",
            "5.25 ft in ft,in",
            "5 ft 2 in in mi,ft",
            "1,000 m to ft,in",
            "7502 s -> [h, min, s]",
            "7502 s -> h,min",
        ] {
            let ParsedLine::Expression(ParsedExpr::UnOpPost { op, .. }) = parse(input) else {
                panic!("{input:?} should end in a conversion");
            };
            assert!(
                matches!(op, OpPost::Ladder(_)),
                "{input:?} should target a ladder"
            );
        }

        // A single unit, bracketed or not, is an ordinary conversion.
        for input in ["1 h to min", "5 m in [km]", "5 m + 3 in -> cm"] {
            let ParsedLine::Expression(ParsedExpr::UnOpPost { op, .. }) = parse(input) else {
                panic!("{input:?} should end in a conversion");
            };
            assert!(
                matches!(op, OpPost::Convert(_)),
                "{input:?} should target one dimension"
            );
        }
    }

    #[test]
    fn parses_implicit_multiplication() {
        parse("5(2)(3)");
        parse("2pi");
    }

    #[test]
    fn variable_multiplication_requires_number_first_and_no_space() {
        parse("5m2");
        assert!(parse_line("5 m2").is_err());
        assert!(parse_line("m2 5").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_line("5 +").is_err());
        assert!(parse_line("@#$").is_err());
    }

    #[test]
    fn reports_unfinished_assignments_as_incomplete() {
        for input in ["x =", "x =   ", "letter =", "let result = "] {
            let error = parse_line(input).expect_err("assignment should be unfinished");
            assert_eq!(error.message(), "Incomplete expression");
        }

        let error = parse_line("5 =").expect_err("number is not a valid assignment name");
        assert_eq!(error.message(), "Unexpected '='");
    }
}
