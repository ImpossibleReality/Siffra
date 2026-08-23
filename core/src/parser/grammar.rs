use super::lexical::{Boxed, Extra, keyword, number_parser, to_span, word_boundary};
use super::representation::*;
use super::span::Span;
use super::temporal;
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
    "after",
    "before",
    "from",
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

pub(super) const KNOWN_FUNCS: &[&str] = &[
    "log", "ln", "sqrt", "sin", "cos", "tan", "asin", "acos", "atan", "root", "exp", "abs",
    "round", "floor", "ceil", "trunc", "sign", "sinh", "cosh", "tanh", "gamma",
];

/// What a `->` step asks for: one unit, or a ladder of them.
enum ArrowTarget {
    Single(ParsedDimension),
    Ladder(Vec<ParsedDimension>),
}

pub(super) fn line_parser<'a>() -> impl Parser<'a, &'a str, ParsedLine, Extra<'a>> {
    let sp: Boxed<'a, ()> = just(' ').repeated().boxed();
    let sp1: Boxed<'a, ()> = just(' ').repeated().at_least(1).boxed();

    let word_boundary = word_boundary();

    let kw = |w: &'static str| keyword(w);

    let number_str = number_parser();

    let number_atom: Boxed<'a, ParsedExpr> = number_str
        .clone()
        .map_with(|value, e| ParsedExpr::Number {
            value,
            units: None,
            span: to_span(e.span()),
        })
        .boxed();

    let instant_literal = temporal::instant_literal();
    let clock_literal = temporal::clock_literal();
    let temporal_literal = temporal::temporal_literal();

    let super::units::UnitParsers {
        mixed_literal,
        quoted_length,
        dimensional_number,
        dollar_number,
        conversion_keyword,
        convert,
        ladder_target,
        convert_ladder,
        units,
    } = super::units::parsers(sp.clone(), sp1.clone(), number_str.clone());

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
            Shift(Op, ParsedExpr, Span),
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

        // Calendar shifts read from the duration toward their reference point:
        // `3 days after Friday` lowers to `Friday + 3 days`. `from` is an
        // alias for `after`, particularly useful with `now`.
        let shift = sp1
            .clone()
            .ignore_then(choice((
                kw("after").to(Op::Add),
                kw("before").to(Op::Subtract),
                kw("from").to(Op::Add),
            )))
            .then_ignore(sp1.clone())
            .then(temporal_literal.clone())
            .map_with(|(op, reference), e| MulTail::Shift(op, reference, to_span(e.span())))
            .boxed();

        // Conversion is a postfix at multiplication precedence.
        let mul_tail = choice((
            mul_op
                .clone()
                .then(product.clone())
                .map(|((op, span), rhs)| MulTail::Arithmetic(op, span, rhs)),
            shift,
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
                        // Keep the duration intact. Evaluation applies any
                        // mixed calendar rungs after resolving its value.
                        MulTail::Shift(op, reference, span) => ParsedExpr::BinOp {
                            lhs: Box::new(reference),
                            op,
                            rhs: Box::new(lhs),
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
