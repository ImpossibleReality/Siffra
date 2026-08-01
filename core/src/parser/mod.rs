pub mod representation;
mod span;
mod syntax;

pub use representation::*;
pub use span::Span;
pub use syntax::{SyntaxKind, SyntaxSpan, syntax_spans};

use crate::error::SiffraExecutionError;
use crate::siffra_error;
use chumsky::error::{RichPattern, RichReason};
use chumsky::prelude::*;
use chumsky::text;

/// Names that are treated as operators / keywords and therefore may not be used
/// as bare variable identifiers.
const RESERVED_VARS: &[&str] = &[
    "plus", "minus", "times", "of", "over", "by", "divided", "divide", "in", "as", "to", "into",
    "let", "on", "off", "what", "is",
];

/// Functions that may be called without parentheses, such as `sin 5` or
/// `log 100`.
const KNOWN_FUNCS: &[&str] = &[
    "log", "ln", "sqrt", "sin", "cos", "tan", "asin", "acos", "atan", "root", "exp", "abs",
    "round", "floor", "ceil", "trunc", "sign", "sinh", "cosh", "tanh", "gamma",
];

/// Words that look like unit names but are actually operators/keywords, so they
/// must never be consumed as a unit. `in` is intentionally absent because it is
/// also a valid unit (inches).
const UNIT_EXCLUDE: &[&str] = &[
    "plus", "minus", "times", "of", "over", "by", "divided", "divide", "as", "to", "into", "on",
    "off", "what", "is",
];

/// Replaces comment contents with spaces while preserving character offsets, so
/// error spans still line up with the input.
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

/// The error type produced by the line parser. `Rich` carries the custom
/// messages that [`describe_error`] turns into user-facing text.
type Extra<'a> = extra::Err<Rich<'a, char>>;
type Boxed<'a, O> = chumsky::Boxed<'a, 'a, &'a str, O, Extra<'a>>;

/// Converts a chumsky span into this crate's own [`Span`].
fn to_span(span: SimpleSpan<usize>) -> Span {
    Span::new(span.start, span.end)
}

/// The numeric literal grammar shared by evaluation and tolerant syntax
/// analysis. Keeping it here prevents highlighting from drifting away from
/// what the calculator accepts (notably grouped digits and exponents).
fn number_parser<'a>() -> Boxed<'a, String> {
    let digit = any().filter(|c: &char| c.is_ascii_digit());
    // Underscores may separate arbitrary digits. Commas, however, must form
    // conventional three-digit groups so an argument separator in `min(1,2)`
    // can never be swallowed as part of a number.
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
/// analysis. Chemical validity is checked later by `Compound`; this parser is
/// intentionally concerned only with the formula's syntactic extent.
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

/// Builds the parser for a single line of input.
fn line_parser<'a>() -> impl Parser<'a, &'a str, ParsedLine, Extra<'a>> {
    let sp: Boxed<'a, ()> = just(' ').repeated().boxed();
    let sp1: Boxed<'a, ()> = just(' ').repeated().at_least(1).boxed();

    // Succeeds (without consuming) when the next char is not part of an
    // identifier, so keywords like `plus` don't match inside `plush`.
    let word_boundary: Boxed<'a, ()> = any()
        .filter(|c: &char| !(c.is_ascii_alphanumeric() || *c == '_'))
        .rewind()
        .ignored()
        .or(end())
        .boxed();

    // Matches a whole keyword, requiring a word boundary afterwards.
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
    // Do not accept a unit as a prefix of an identifier. In particular,
    // `5 m2` must not become the dimensional value `5 m` multiplied by `2`;
    // the variable shorthand is intentionally the adjacent `5m2` form.
    .then_ignore(word_boundary.clone())
    .try_map(|s: String, span: SimpleSpan| {
        if UNIT_EXCLUDE.contains(&s.as_str()) {
            Err(Rich::custom(span, "operator word, not a unit"))
        } else {
            Ok(s)
        }
    })
    .boxed();

    // A chemical formula, e.g. `H2O`, `NaCl`, `Ca(OH)2`. Captured verbatim.
    let chemical_str = chemical_parser();

    // A chemical qualifier is part of a single unit atom, so the natural
    // `g H2O` form is unambiguous even though it contains one space. Brackets
    // remain available for compound unit expressions such as `[g H2O / L]`.
    let unit_atom: Boxed<'a, ParsedUnit> = unit_name
        .clone()
        .then(sp1.clone().ignore_then(chemical_str.clone()).or_not())
        .map_with(|(name, chemical), e| ParsedUnit {
            name,
            chemical,
            span: to_span(e.span()),
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

    let units: Boxed<'a, ParsedDimension> =
        choice((symbolic_units, bracketed_units, bare_units)).boxed();

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

    let convert: Boxed<'a, ParsedDimension> = sp1
        .clone()
        .ignore_then(choice((kw("into"), kw("in"), kw("as"), kw("to"))))
        .ignore_then(sp1.clone())
        .ignore_then(units.clone())
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

        // An implicit product has ordinary multiplication precedence, rather
        // than atom precedence: `5m^2` is `5 * (m^2)`, not `(5m)^2`.
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

            // Prefix negation is accepted on either side of `^`. Keeping the
            // recursive operand at the unary level preserves right
            // associativity
            // (`2^3^2`) and makes negative exponents (`2^-3`) natural, while
            // `-2^2` continues to mean `-(2^2)`.
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

            // Implicit multiplication of directly-adjacent terms: `5(x)(y)`,
            // `2pi`, and `5m^2`.
            // A following minus belongs to subtraction, not to an implicitly
            // multiplied negative factor (`4 - 2` must not become `4 * -2`).
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
        }

        // Arithmetic and conversion postfixes share one chain. A conversion
        // therefore applies to everything before it, while a following `* 2`
        // can continue operating on the converted value. Contiguous `on`/`off`
        // percentage applications are folded separately below, because their
        // natural-language reading is right-associative.
        let mul_tail = choice((
            mul_op
                .clone()
                .then(product.clone())
                .map(|((op, span), rhs)| MulTail::Arithmetic(op, span, rhs)),
            convert
                .clone()
                .map_with(|dim, e| MulTail::Convert(dim, to_span(e.span()))),
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

    // End-of-line conversion: `<expr> -> unit` converts the *entire* line to
    // `unit`. Unlike the `in`/`as`/`to` postfix (which binds at the `mul`
    // level), this binds looser than everything, so `5 m + 3 in -> cm`
    // converts the whole sum without parentheses. Because it lives at the line
    // level, it is only ever valid at the very end of the line. For an
    // assignment it wraps the right-hand side, so the stored value is converted
    // too.
    // The arrow's target unit must be the last thing on the line. We capture
    // any remaining characters so a trailing `+ 2` (etc.) can be reported
    // against the arrow itself, rather than surfacing a generic "unexpected
    // character" further along.
    let rest_of_line: Boxed<'a, String> = any().repeated().collect::<String>().boxed();

    let arrow_conversion = sp
        .clone()
        .ignore_then(just("->").map_with(|_, e| to_span(e.span())))
        .then_ignore(sp.clone())
        .then(units.clone())
        .then_ignore(sp.clone())
        .then(rest_of_line)
        // The error is *emitted* rather than returned: chumsky throws away the
        // errors of a branch it backtracks out of, so failing here would let
        // the optional arrow quietly match nothing and surface an unhelpful
        // "unexpected '>'" from the subtraction attempt instead.
        .validate(
            |((arrow_span, dim), rest): ((Span, ParsedDimension), String), _, emitter| {
                if !rest.is_empty() {
                    emitter.emit(Rich::custom(
                        SimpleSpan::from(arrow_span.start()..arrow_span.end()),
                        "`-> [Unit]` must be at the end of a line",
                    ));
                }
                (dim, arrow_span)
            },
        )
        .or_not()
        .boxed();

    let line_expr: Boxed<'a, ParsedExpr> = expr
        .clone()
        .then(arrow_conversion)
        .map(|(e, conversion)| match conversion {
            Some((dim, arrow_span)) => {
                let span = arrow_span.merge(dim.span);
                ParsedExpr::UnOpPost {
                    lhs: Box::new(e),
                    op: OpPost::Convert(dim),
                    span,
                }
            }
            None => e,
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

/// Reads a line as a header — a title line introduced by one or more leading
/// `#` markers, as in `# Trip budget`. Returns the title with the markers and
/// surrounding whitespace stripped; an empty title (`#` on its own) still
/// counts as a header.
///
/// `#` is also a scalar unit alias (`5 #`), but a unit cannot open a line, so a
/// leading `#` is unambiguous.
pub fn parse_header(input: &str) -> Option<String> {
    let rest = input.trim_start().strip_prefix('#')?;
    Some(rest.trim_start_matches('#').trim().to_string())
}

/// Parses a single line into a [`ParsedLine`].
///
/// Blank lines and comment-only lines produce [`ParsedLine::Comment`], and
/// `#`-prefixed lines produce [`ParsedLine::Header`]. Syntax errors are
/// reported as [`SiffraExecutionError`] values with spans pointing at the
/// offending characters.
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
            // Prefer the error that made the most progress through the input.
            let err = errs
                .into_iter()
                .max_by_key(|e| e.span().end)
                .expect("parse failure always yields at least one error");
            let span = *err.span();
            let (message, description) = describe_error(&err, &cleaned);
            siffra_error!(message, description, Some(Span::new(span.start, span.end)))
        })
}

/// Turns a chumsky error into a human-friendly (message, description) pair.
/// The message is shown inline; the description appears in the error popover.
fn describe_error(err: &Rich<'_, char>, input: &str) -> (String, String) {
    // Chumsky may report the assignment operator as the unexpected token when
    // the right-hand side has not been entered yet.  From a user's perspective
    // the assignment is valid so far; it is just unfinished.
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
                // Ran out of input part-way through an expression. Chumsky no
                // longer tracks delimiters itself, so an unclosed bracket shows
                // up as a closing delimiter among the expected tokens.
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

/// The closing delimiter an expected-token pattern stands for, if it is one.
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
        // `#` is still a scalar unit when it follows a number.
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
        // Binds looser than `+`, so the conversion wraps the entire sum without
        // needing parentheses.
        let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) =
            parse("5 m + 3 in -> cm")
        else {
            panic!("`->` should wrap the whole line expression");
        };
        assert!(matches!(op, OpPost::Convert(_)));
        assert!(matches!(*lhs, ParsedExpr::BinOp { op: Op::Add, .. }));

        // Also works without spaces and on assignments, converting the stored
        // value.
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
        // Not a general infix operator: it may not appear mid-expression.
        assert!(parse_line("(5 -> cm) + 2").is_err());

        // Trailing content after the target unit is reported against the arrow
        // with a descriptive message.
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
