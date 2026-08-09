use super::SiffraState;
use crate::error::SiffraExecutionError;
use crate::representations::{
    Basis, ClockLiteral, Granularity, InstantKeyword, InstantLiteral, TimeZone, Value,
};
use crate::siffra_error;
use jiff::civil::Date;
use jiff::{Span as JiffSpan, Zoned};

/// Resolves a written instant against the pass's zone and timestamp. DST edge
/// cases follow jiff's compatible disambiguation (skip forward over a gap, take
/// the earlier of an ambiguous repeat), which matches RFC 5545.
pub(super) fn resolve_instant(
    literal: &InstantLiteral,
    state: &SiffraState,
    span: Option<crate::parser::Span>,
) -> Result<Value, SiffraExecutionError> {
    let zone = literal
        .zone
        .clone()
        .unwrap_or_else(|| state.timezone().clone());

    if let Some(keyword) = literal.keyword {
        let now = state.now().with_time_zone(zone.inner().clone());
        if let Some((hour, minute, second)) = literal.time {
            let day_offset = match keyword {
                InstantKeyword::Today | InstantKeyword::Now => 0,
                InstantKeyword::Tomorrow => 1,
                InstantKeyword::Yesterday => -1,
            };
            let date = start_of_day(&now, day_offset, span)?.date();
            let moment = date
                .at(hour, minute, second, 0)
                .to_zoned(zone.inner().clone())
                .map_err(|_| {
                    siffra_error!(
                        "Date Error",
                        format!("This time does not exist in {}", zone.label()),
                        span
                    )
                })?;
            let granularity = if literal.has_seconds {
                Granularity::Second
            } else {
                Granularity::Minute
            };
            return Ok(Value::instant(&moment, &zone, granularity));
        }
        let (moment, granularity) = match keyword {
            InstantKeyword::Now => (now, Granularity::Second),
            InstantKeyword::Today => (start_of_day(&now, 0, span)?, Granularity::Date),
            InstantKeyword::Tomorrow => (start_of_day(&now, 1, span)?, Granularity::Date),
            InstantKeyword::Yesterday => (start_of_day(&now, -1, span)?, Granularity::Date),
        };
        return Ok(Value::instant(&moment, &zone, granularity));
    }

    let (written_year, month, day) = literal.date.expect("a literal has a date or a keyword");
    let invalid_date = |year: i16| {
        siffra_error!(
            "Date Error",
            format!("{year:04}-{month:02}-{day:02} is not a date on the calendar"),
            span
        )
    };
    let date = match written_year {
        Some(year) => Date::new(year, month, day).map_err(|_| invalid_date(year))?,
        None => {
            let today = state.now().with_time_zone(zone.inner().clone()).date();
            match Date::new(today.year(), month, day) {
                Ok(candidate) if candidate >= today => candidate,
                _ => {
                    let next_year = today.year() + 1;
                    Date::new(next_year, month, day).map_err(|_| invalid_date(next_year))?
                }
            }
        }
    };

    let (hour, minute, second) = literal.time.unwrap_or((0, 0, 0));
    let moment = date
        .at(hour, minute, second, 0)
        .to_zoned(zone.inner().clone())
        .map_err(|_| {
            siffra_error!(
                "Date Error",
                format!("This time does not exist in {}", zone.label()),
                span
            )
        })?;

    let granularity = match (literal.time, literal.has_seconds) {
        (None, _) => Granularity::Date,
        (Some(_), false) => Granularity::Minute,
        (Some(_), true) => Granularity::Second,
    };
    Ok(Value::instant(&moment, &zone, granularity))
}

/// Resolves a wall clock against the pass's zone. The date it is anchored to
/// is today's in that zone, which is what decides the offset a conversion to
/// another zone reads it through.
pub(super) fn resolve_clock(
    literal: &ClockLiteral,
    state: &SiffraState,
    span: Option<crate::parser::Span>,
) -> Result<Value, SiffraExecutionError> {
    let zone = literal
        .zone
        .clone()
        .unwrap_or_else(|| state.timezone().clone());
    let (hour, minute, second) = literal.time;
    let moment = state
        .now()
        .with_time_zone(zone.inner().clone())
        .date()
        .at(hour, minute, second, 0)
        .to_zoned(zone.inner().clone())
        .map_err(|_| {
            siffra_error!(
                "Time Error",
                format!("This time does not exist today in {}", zone.label()),
                span
            )
        })?;
    let granularity = if literal.has_seconds {
        Granularity::Second
    } else {
        Granularity::Minute
    };
    Ok(Value::clock(&moment, &zone, granularity))
}

pub(super) fn start_of_day(
    now: &Zoned,
    day_offset: i64,
    span: Option<crate::parser::Span>,
) -> Result<Zoned, SiffraExecutionError> {
    let shifted = if day_offset == 0 {
        now.clone()
    } else {
        now.checked_add(JiffSpan::new().days(day_offset))
            .map_err(|_| siffra_error!("Date Error", "That day is off the calendar", span))?
    };
    shifted
        .start_of_day()
        .map_err(|_| siffra_error!("Date Error", "That day has no start", span))
}

/// Instants read back at the granularity their wall clock justifies: a value
/// that lands on midnight is a date, otherwise it needs a clock.
pub(super) fn granularity_of(moment: &Zoned) -> Granularity {
    let time = moment.time();
    if time.second() != 0 || time.subsec_nanosecond() != 0 {
        Granularity::Second
    } else if time.hour() != 0 || time.minute() != 0 {
        Granularity::Minute
    } else {
        Granularity::Date
    }
}

pub(super) fn anchored_endpoints(
    value: &Value,
    function: &str,
    span: Option<crate::parser::Span>,
) -> Result<(Zoned, Zoned), SiffraExecutionError> {
    let anchors = value.basis().and_then(Basis::anchors).ok_or_else(|| {
        siffra_error!(
            "Range Error",
            format!("{function} needs a single date range (this value is not one)"),
            span
        )
    })?;
    Ok((anchors.0.clone(), anchors.1.clone()))
}

pub(super) fn instant_from(moment: &Zoned) -> Value {
    let zone = moment
        .time_zone()
        .iana_name()
        .and_then(TimeZone::parse)
        .unwrap_or_else(TimeZone::utc);
    Value::instant(moment, &zone, granularity_of(moment))
}
