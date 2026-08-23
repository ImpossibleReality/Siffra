//! Grammar for the date, time and timezone literals.
//!
//! These are digit-led, so each one has to win over the arithmetic reading of
//! the same characters: `2024-02-10` is a date rather than three subtractions,
//! and `9:00` is a wall clock rather than a bare `9`. Callers get that by
//! trying these parsers ahead of the numeric atom.

use super::Span;
use super::lexical::{Boxed, to_span};
use super::representation::{
    ParsedDate, ParsedDimension, ParsedExpr, ParsedInstant, ParsedTime, ParsedUnit,
};
use crate::representations::{InstantKeyword, TimeZone};
use chumsky::prelude::*;

const INSTANT_KEYWORDS: &[(&str, InstantKeyword)] = &[
    ("now", InstantKeyword::Now),
    ("today", InstantKeyword::Today),
    ("tomorrow", InstantKeyword::Tomorrow),
    ("yesterday", InstantKeyword::Yesterday),
];

/// Controls the two locale-dependent components in numeric dates. Four-digit
/// year-first dates remain ISO ordered regardless of this setting.
const MONTH_FIRST_IN_NUMERIC_DATES: bool = true;
const TWO_DIGIT_YEAR_BASE: u32 = 2000;

fn spaces<'a>() -> Boxed<'a, ()> {
    just(' ').repeated().at_least(1).boxed()
}

fn keyword<'a>(word: &'static str) -> Boxed<'a, &'a str> {
    just(word)
        .then_ignore(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_')
                .not()
                .rewind()
                .or(end()),
        )
        .boxed()
}

fn digits_value<'a>(least: usize, most: usize) -> Boxed<'a, u32> {
    any()
        .filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .at_least(least)
        .at_most(most)
        .to_slice()
        .map(|digits: &str| digits.parse::<u32>().expect("bounded digit run"))
        // A longer digit run is a different literal, not a truncated one.
        .then_ignore(
            any()
                .filter(|c: &char| c.is_ascii_digit())
                .not()
                .rewind()
                .or(end()),
        )
        .boxed()
}

fn written_year<'a>() -> Boxed<'a, i16> {
    choice((
        digits_value(4, 4),
        digits_value(2, 2).map(|year| TWO_DIGIT_YEAR_BASE + year),
    ))
    .map(|year| year as i16)
    .boxed()
}

fn numeric_date<'a>(separator: char) -> Boxed<'a, ParsedDate> {
    let iso = digits_value(4, 4)
        .then_ignore(just(separator))
        .then(digits_value(1, 2))
        .then_ignore(just(separator))
        .then(digits_value(1, 2))
        .map(|((year, month), day)| ParsedDate {
            year: Some(year as i16),
            month: month as i8,
            day: day as i8,
        });

    let localized = digits_value(1, 2)
        .then_ignore(just(separator))
        .then(digits_value(1, 2))
        .then_ignore(just(separator))
        .then(written_year())
        .map(|((first, second), year)| {
            let (month, day) = if MONTH_FIRST_IN_NUMERIC_DATES {
                (first, second)
            } else {
                (second, first)
            };
            ParsedDate {
                year: Some(year),
                month: month as i8,
                day: day as i8,
            }
        });

    choice((iso, localized)).boxed()
}

fn month_name<'a>() -> Boxed<'a, u32> {
    text::ascii::ident()
        .try_map(|name: &str, span| {
            let month = match name.to_ascii_lowercase().as_str() {
                "jan" | "january" => 1,
                "feb" | "february" => 2,
                "mar" | "march" => 3,
                "apr" | "april" => 4,
                "may" => 5,
                "jun" | "june" => 6,
                "jul" | "july" => 7,
                "aug" | "august" => 8,
                "sep" | "sept" | "september" => 9,
                "oct" | "october" => 10,
                "nov" | "november" => 11,
                "dec" | "december" => 12,
                _ => return Err(Rich::custom(span, "not a month name")),
            };
            Ok(month)
        })
        .boxed()
}

fn optional_named_year<'a>() -> Boxed<'a, Option<i16>> {
    choice((
        just(',').ignore_then(spaces()).ignore_then(written_year()),
        spaces().ignore_then(written_year()),
    ))
    .or_not()
    .boxed()
}

fn named_date<'a>() -> Boxed<'a, ParsedDate> {
    let month_first = month_name()
        .then_ignore(spaces())
        .then(digits_value(1, 2))
        .then(optional_named_year())
        .map(|((month, day), year)| ParsedDate {
            year,
            month: month as i8,
            day: day as i8,
        });

    let day_first = digits_value(1, 2)
        .then_ignore(spaces())
        .then(month_name())
        .then(optional_named_year())
        .map(|((day, month), year)| ParsedDate {
            year,
            month: month as i8,
            day: day as i8,
        });

    choice((month_first, day_first)).boxed()
}

/// Numeric dates require a year so `2/11` and `2-11` retain their arithmetic
/// meanings. A named month may omit its year because it is unambiguous.
fn date<'a>() -> Boxed<'a, ParsedDate> {
    choice((numeric_date('-'), numeric_date('/'), named_date())).boxed()
}

fn meridiem<'a>() -> Boxed<'a, bool> {
    choice((
        one_of("aA").then(one_of("mM").or_not()).to(false),
        one_of("pP").then(one_of("mM").or_not()).to(true),
    ))
    .then_ignore(
        any()
            .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_')
            .not()
            .rewind()
            .or(end()),
    )
    .boxed()
}

fn normalize_time<'a>(
    hour: u32,
    minute: u32,
    second: Option<u32>,
    pm: Option<bool>,
    span: SimpleSpan,
) -> Result<ParsedTime, Rich<'a, char>> {
    let hour = match pm {
        Some(_) if !(1..=12).contains(&hour) => {
            return Err(Rich::custom(
                span,
                "12-hour time must use an hour from 1 to 12",
            ));
        }
        Some(false) if hour == 12 => 0,
        Some(true) if hour != 12 => hour + 12,
        _ => hour,
    };
    Ok(ParsedTime {
        hour: hour as i8,
        minute: minute as i8,
        second: second.map(|second| second as i8),
    })
}

fn time<'a>() -> Boxed<'a, ParsedTime> {
    let colon_time = digits_value(1, 2)
        .then_ignore(just(':'))
        .then(digits_value(2, 2))
        .then(just(':').ignore_then(digits_value(2, 2)).or_not())
        .then(just(' ').or_not().ignore_then(meridiem()).or_not())
        .try_map(|(((hour, minute), second), pm), span| {
            normalize_time(hour, minute, second, pm, span)
        });

    let short_time = digits_value(1, 2)
        .then(meridiem())
        .try_map(|(hour, pm), span| normalize_time(hour, 0, None, Some(pm), span));

    choice((colon_time, short_time)).boxed()
}

/// IANA names, `UTC`, hardcoded abbreviations, and `±HH:MM` offsets.
fn zone_name<'a>() -> Boxed<'a, String> {
    let segment = any()
        .filter(|c: &char| c.is_ascii_uppercase())
        .then(
            any()
                .filter(|c: &char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
                .repeated(),
        )
        .to_slice();

    let iana = segment
        .then(just('/').then(segment).repeated().at_least(1))
        .to_slice()
        .boxed();

    // Only the full `±HH:MM` form, so that `2024-02-10 +1 day` stays addition.
    let offset = one_of("+-")
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit())
                .repeated()
                .exactly(2),
        )
        .then_ignore(just(':'))
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit())
                .repeated()
                .exactly(2),
        )
        .to_slice()
        .boxed();

    let named = choice(
        ["UTC", "GMT"]
            .into_iter()
            .chain(TimeZone::abbreviations())
            .map(just)
            .collect::<Vec<_>>(),
    )
    .then_ignore(
        any()
            .filter(|c: &char| c.is_ascii_alphanumeric() || matches!(c, '_' | '/'))
            .not()
            .rewind()
            .or(end()),
    )
    .to_slice()
    .boxed();

    choice((iana, named, offset))
        .map(|name: &str| name.to_string())
        .try_map(|name: String, span: SimpleSpan| {
            if TimeZone::parse(&name).is_some() {
                Ok(name)
            } else {
                Err(Rich::custom(span, "unknown timezone"))
            }
        })
        .boxed()
}

/// A point on the calendar: a date, optionally with a time and a zone, or one
/// of the keywords that resolve against the evaluation timestamp. A zoned time
/// without an explicit date is anchored to today in that zone.
pub fn instant_literal<'a>() -> Boxed<'a, ParsedExpr> {
    let zone_suffix: Boxed<'a, (String, Span)> = zone_name()
        .map_with(|name, e| (name, to_span(e.span())))
        .boxed();

    let date_first = date()
        .then(spaces().ignore_then(time()).or_not())
        .map(|(date, time)| (None, Some(date), time, None));
    let date_after_time = choice((
        choice((
            spaces()
                .ignore_then(keyword("on").ignored())
                .then_ignore(spaces()),
            spaces(),
        ))
        .ignore_then(date())
        .map(|date| (None, Some(date))),
        spaces()
            .ignore_then(keyword("today"))
            .to((Some(InstantKeyword::Today), None)),
    ));
    // A time with no date of its own is a wall clock, not a point on the
    // calendar, so this branch always ends in one.
    let time_first = time()
        .then(spaces().ignore_then(zone_suffix.clone()).or_not())
        .then(date_after_time)
        .map(|((time, zone), (keyword, date))| (keyword, date, Some(time), zone));

    let dated = choice((date_first, time_first))
        .then(spaces().ignore_then(zone_suffix).or_not())
        .map_with(
            |((keyword, date, time, leading_zone), trailing_zone), e| ParsedExpr::Instant {
                instant: ParsedInstant {
                    keyword,
                    date,
                    time,
                    zone: trailing_zone.or(leading_zone),
                },
                span: to_span(e.span()),
            },
        );

    let keywords = choice(
        INSTANT_KEYWORDS
            .iter()
            .map(|(word, value)| keyword(word).to(*value))
            .collect::<Vec<_>>(),
    )
    .map_with(|keyword, e| ParsedExpr::Instant {
        instant: ParsedInstant {
            keyword: Some(keyword),
            date: None,
            time: None,
            zone: None,
        },
        span: to_span(e.span()),
    });

    choice((dated, keywords)).boxed()
}

/// A wall-clock reading, optionally read in a named zone. Without one it
/// belongs to the document's zone, which is only known during evaluation.
pub fn clock_literal<'a>() -> Boxed<'a, ParsedExpr> {
    time()
        .then(
            spaces()
                .ignore_then(zone_name().map_with(|name, e| (name, to_span(e.span()))))
                .or_not(),
        )
        .map_with(|(time, zone), e| ParsedExpr::ClockTime {
            time,
            zone,
            span: to_span(e.span()),
        })
        .boxed()
}

/// A zone named as a conversion target. A zone is a unit of the Time axis, so
/// relabeling an instant reads as an ordinary conversion.
pub fn zone_dimension<'a>() -> Boxed<'a, ParsedDimension> {
    zone_name()
        .map_with(|name, e| {
            let span = to_span(e.span());
            ParsedDimension {
                numerator: vec![(
                    ParsedUnit {
                        name,
                        chemical: None,
                        span,
                    },
                    "1".to_string(),
                )],
                denominator: Vec::new(),
                span,
            }
        })
        .boxed()
}

/// A date or time written out in full. Tried ahead of a conversion target so
/// that `to dec 1` reads as a date rather than as a decade.
pub fn temporal_literal<'a>() -> Boxed<'a, ParsedExpr> {
    choice((instant_literal(), clock_literal())).boxed()
}
