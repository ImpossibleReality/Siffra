//! Grammar for dimensions, mixed-unit literals, and conversion targets.

use super::lexical::{Boxed, chemical_parser, keyword, to_span, word_boundary};
use super::representation::{ParsedDimension, ParsedExpr, ParsedUnit, SCALAR_UNIT_ALIASES};
use super::temporal;
use chumsky::prelude::*;

/// The words that introduce a conversion target — or, when what follows is not
/// a dimension, the far endpoint of an interval.
const CONVERSION_KEYWORDS: &[&str] = &["into", "in", "as", "to"];

pub(super) const UNIT_EXCLUDE: &[&str] = &[
    "plus", "minus", "times", "of", "over", "by", "divided", "divide", "as", "to", "into", "on",
    "off", "what", "is", "after", "before", "from",
];

pub(super) struct UnitParsers<'a> {
    pub mixed_literal: Boxed<'a, ParsedExpr>,
    pub quoted_length: Boxed<'a, ParsedExpr>,
    pub dimensional_number: Boxed<'a, ParsedExpr>,
    pub dollar_number: Boxed<'a, ParsedExpr>,
    pub conversion_keyword: Boxed<'a, ()>,
    pub convert: Boxed<'a, ParsedDimension>,
    pub ladder_target: Boxed<'a, Vec<ParsedDimension>>,
    pub convert_ladder: Boxed<'a, Vec<ParsedDimension>>,
    pub units: Boxed<'a, ParsedDimension>,
}

pub(super) fn parsers<'a>(
    sp: Boxed<'a, ()>,
    sp1: Boxed<'a, ()>,
    number: Boxed<'a, String>,
) -> UnitParsers<'a> {
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
    .then_ignore(word_boundary())
    .try_map(|name: String, span: SimpleSpan| {
        if UNIT_EXCLUDE.contains(&name.as_str()) {
            Err(Rich::custom(span, "operator word, not a unit"))
        } else {
            Ok(name)
        }
    })
    .boxed();

    let unit_atom: Boxed<'a, ParsedUnit> = unit_name
        .clone()
        .then(sp1.clone().ignore_then(chemical_parser()).or_not())
        .map_with(|(name, chemical), e| ParsedUnit {
            name,
            chemical,
            span: to_span(e.span()),
        })
        .boxed();

    // A rung is a bare unit name: no chemical qualifier, no power, no product.
    let plain_unit: Boxed<'a, ParsedUnit> = unit_name
        .map_with(|name, e| ParsedUnit {
            name,
            chemical: None,
            span: to_span(e.span()),
        })
        .boxed();

    let mixed_rung: Boxed<'a, (String, ParsedUnit)> = number
        .clone()
        .then_ignore(sp1.clone())
        .then(plain_unit.clone())
        .boxed();

    // `5 ft 2 in` is one length. The rungs have to agree on what they measure
    // and shrink left to right, or the line remains an implicit product.
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

    // Feet and inches shorthand (e.g. 5'2").
    let quoted_length: Boxed<'a, ParsedExpr> = number
        .clone()
        .then_ignore(just('\''))
        .then(number.clone())
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
        .then(number.clone())
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
        .map(|(unit, power)| (unit, power.unwrap_or_else(|| "1".to_string())))
        .boxed();

    let bare_unit_mul_group: Boxed<'a, Vec<(ParsedUnit, String)>> = bare_unit_pow
        .clone()
        .then(
            just('*')
                .ignore_then(bare_unit_pow)
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| {
            let mut units = vec![first];
            units.extend(rest);
            units
        })
        .boxed();

    // Bare units are compact: no whitespace around `*`, `/`, or `^`. A leading
    // `1/` is accepted only for denominator-only dimensions emitted by Display.
    let ordinary_bare_units = bare_unit_mul_group
        .clone()
        .then(just('/').ignore_then(bare_unit_mul_group.clone()).or_not())
        .map(|(numerator, denominator)| (numerator, denominator.unwrap_or_default()))
        .boxed();
    let inverse_bare_units = just('1')
        .then_ignore(just('/'))
        .ignore_then(bare_unit_mul_group)
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
        .map(|(unit, power)| (unit, power.unwrap_or_else(|| "1".to_string())))
        .boxed();

    let bracketed_unit_mul_group = bracketed_unit_pow
        .clone()
        .then(
            just('*')
                .padded_by(sp.clone())
                .ignore_then(bracketed_unit_pow)
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(first, rest)| {
            let mut units = vec![first];
            units.extend(rest);
            units
        })
        .boxed();

    let bracketed_unit_group: Boxed<'a, Vec<(ParsedUnit, String)>> = choice((
        bracketed_unit_mul_group,
        just('1').map(|_| Vec::<(ParsedUnit, String)>::new()),
    ))
    .boxed();

    let bracketed_units: Boxed<'a, ParsedDimension> = bracketed_unit_group
        .clone()
        .then(
            just('/')
                .padded_by(sp.clone())
                .ignore_then(bracketed_unit_group)
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

    let dimensional_number: Boxed<'a, ParsedExpr> = number
        .clone()
        .then(sp1.clone().ignore_then(units.clone()))
        .map_with(|(value, units), e| ParsedExpr::Number {
            value,
            units: Some(units),
            span: to_span(e.span()),
        })
        .boxed();

    let dollar_number: Boxed<'a, ParsedExpr> = just('$')
        .ignore_then(number)
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
                .map(|word| keyword(word))
                .collect::<Vec<_>>(),
        ))
        .ignore_then(sp1)
        .ignored()
        .boxed();

    // Only known units form conversion targets; other names can then be read
    // as interval endpoints by the expression grammar.
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
    let convert = conversion_keyword.clone().ignore_then(known_units).boxed();

    let rung: Boxed<'a, ParsedDimension> = plain_unit
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
        .separated_by(just(',').padded_by(sp.clone()))
        .at_least(2)
        .collect::<Vec<_>>()
        .padded_by(sp)
        .delimited_by(just('['), just(']'))
        .boxed();
    let ladder_target = choice((bracketed_ladder, bare_ladder)).boxed();
    let convert_ladder = conversion_keyword
        .clone()
        .ignore_then(ladder_target.clone())
        .boxed();

    UnitParsers {
        mixed_literal,
        quoted_length,
        dimensional_number,
        dollar_number,
        conversion_keyword,
        convert,
        ladder_target,
        convert_ladder,
        units,
    }
}

fn is_known_unit(name: &str) -> bool {
    SCALAR_UNIT_ALIASES.contains(&name) || crate::representations::Unit::from_str(name).is_ok()
}

fn is_mixed_literal(rungs: &[(String, ParsedUnit)]) -> bool {
    let units = rungs
        .iter()
        .map(|(_, unit)| crate::representations::Unit::from_str(&unit.name).ok())
        .collect::<Option<Vec<_>>>();
    units.is_some_and(|units| crate::representations::ladder::validate(&units).is_ok())
}
