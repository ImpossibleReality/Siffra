use crate::representations::{Float, Time, TimeZone};
use jiff::{Timestamp, Unit as JiffUnit, Zoned};
use std::ops::Neg;

/// Readings of a duration on each irregular calendar axis, plus the exact
/// elapsed seconds every sub-day unit is derived from. Everything else follows:
/// weeks are days/7, decades are years/10, hours are seconds/3600.
///
/// `seconds` is carried here rather than read off the value's own scalar so
/// that a span keeps answering from its endpoints after being converted:
/// `(a to b) -> day -> hour` is 695, not 29 × 24.
#[derive(Debug, Clone, PartialEq)]
pub struct CalendarReadings {
    pub years: Float,
    pub months: Float,
    pub days: Float,
    pub seconds: Float,
}

impl CalendarReadings {
    fn zip(&self, other: &Self, combine: impl Fn(&Float, &Float) -> Float) -> Self {
        Self {
            years: combine(&self.years, &other.years),
            months: combine(&self.months, &other.months),
            days: combine(&self.days, &other.days),
            seconds: combine(&self.seconds, &other.seconds),
        }
    }

    fn map(&self, transform: impl Fn(&Float) -> Float) -> Self {
        Self {
            years: transform(&self.years),
            months: transform(&self.months),
            days: transform(&self.days),
            seconds: transform(&self.seconds),
        }
    }

    pub fn added(&self, other: &Self) -> Self {
        self.zip(other, |a, b| a + b)
    }

    pub fn subtracted(&self, other: &Self) -> Self {
        self.zip(other, |a, b| a - b)
    }

    pub fn scaled(&self, factor: &Float) -> Self {
        self.map(|value| value * factor)
    }

    pub fn negated(&self) -> Self {
        self.map(|value| value.clone().neg())
    }

    /// Projects onto one calendar unit. Regular multiples of years, months and
    /// days are derived here rather than being read separately, so they always
    /// agree with the axis they come from.
    pub fn project(&self, unit: Time) -> Option<Float> {
        let scaled = |source: &Float, divisor: i64| source / &Float::from(divisor);
        Some(match unit {
            Time::Day => self.days.clone(),
            Time::Week => scaled(&self.days, 7),
            Time::Fortnight => scaled(&self.days, 14),
            Time::Month => self.months.clone(),
            Time::Year => self.years.clone(),
            Time::Decade => scaled(&self.years, 10),
            Time::Century => scaled(&self.years, 100),
            Time::Millennium => scaled(&self.years, 1000),
            _ => return None,
        })
    }
}

/// Semantic side-data upgrading a duration from "mean-length approximation" to
/// "calendar-exact". A single comparison keeps its endpoints and stays fully
/// capable; combining comparisons degrades to readings, because a formal sum of
/// spans has no pair of endpoints that represents it.
#[derive(Debug, Clone, PartialEq)]
pub enum Basis {
    Span { start: Zoned, end: Zoned },
    CalendarReadings(CalendarReadings),
}

impl Basis {
    pub fn span(start: Zoned, end: Zoned) -> Self {
        Basis::Span { start, end }
    }

    /// Readings are measured relative to the start endpoint, in its own zone,
    /// so DST and leap years are handled by tzdata rather than by us.
    pub fn readings(&self) -> CalendarReadings {
        match self {
            Basis::CalendarReadings(readings) => readings.clone(),
            Basis::Span { start, end } => {
                let seconds = &epoch_seconds(end) - &epoch_seconds(start);
                if seconds == Float::from(0) {
                    return CalendarReadings {
                        years: Float::from(0),
                        months: Float::from(0),
                        days: Float::from(0),
                        seconds,
                    };
                }
                let Ok(span) = start.until(end) else {
                    return CalendarReadings {
                        years: Float::from(0),
                        months: Float::from(0),
                        days: Float::from(0),
                        seconds,
                    };
                };
                let total = |unit: JiffUnit| {
                    span.total((unit, start))
                        .ok()
                        .and_then(float_from_f64)
                        .unwrap_or_else(|| Float::from(0))
                };
                CalendarReadings {
                    years: total(JiffUnit::Year),
                    months: total(JiffUnit::Month),
                    days: total(JiffUnit::Day),
                    seconds,
                }
            }
        }
    }

    pub fn anchors(&self) -> Option<(&Zoned, &Zoned)> {
        match self {
            Basis::Span { start, end } => Some((start, end)),
            Basis::CalendarReadings(_) => None,
        }
    }

    pub fn added(&self, other: &Self) -> Self {
        Basis::CalendarReadings(self.readings().added(&other.readings()))
    }

    pub fn subtracted(&self, other: &Self) -> Self {
        Basis::CalendarReadings(self.readings().subtracted(&other.readings()))
    }

    pub fn scaled(&self, factor: &Float) -> Self {
        Basis::CalendarReadings(self.readings().scaled(factor))
    }

    pub fn negated(&self) -> Self {
        match self {
            Basis::Span { start, end } => Basis::Span {
                start: end.clone(),
                end: start.clone(),
            },
            Basis::CalendarReadings(readings) => Basis::CalendarReadings(readings.negated()),
        }
    }
}

/// jiff totals are `f64`; the shortest round-trip spelling keeps `36.15` from
/// rendering as its binary expansion at 256-bit precision.
fn float_from_f64(value: f64) -> Option<Float> {
    if !value.is_finite() {
        return None;
    }
    Float::parse(&value.to_string()).ok()
}

/// Epoch seconds as a `Float`, with the fractional part carried in nanoseconds.
pub fn zoned_from_epoch_seconds(value: &Float, zone: &TimeZone) -> Option<Zoned> {
    let (negative, integer, fraction) = value.decimal_digits()?;
    let magnitude = integer.parse::<i64>().ok()?;

    let mut nanoseconds = fraction
        .chars()
        .chain(std::iter::repeat('0'))
        .take(9)
        .collect::<String>()
        .parse::<i32>()
        .ok()?;
    let mut seconds = if negative { -magnitude } else { magnitude };
    if negative && nanoseconds != 0 {
        seconds -= 1;
        nanoseconds = 1_000_000_000 - nanoseconds;
    }

    let timestamp = Timestamp::new(seconds, nanoseconds).ok()?;
    Some(timestamp.to_zoned(zone.inner().clone()))
}

pub fn epoch_seconds(zoned: &Zoned) -> Float {
    let timestamp = zoned.timestamp();
    let seconds = Float::from(timestamp.as_second());
    let nanoseconds = timestamp.subsec_nanosecond();
    if nanoseconds == 0 {
        return seconds;
    }
    &seconds + &(&Float::from(nanoseconds as i64) / &Float::from(1_000_000_000i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    fn zone() -> TimeZone {
        TimeZone::parse("America/Los_Angeles").unwrap()
    }

    fn at(year: i16, month: i8, day: i8, hour: i8) -> Zoned {
        date(year, month, day)
            .at(hour, 0, 0, 0)
            .to_zoned(zone().inner().clone())
            .unwrap()
    }

    #[test]
    fn readings_come_from_the_real_endpoints_not_the_scalar() {
        let noon = Basis::span(at(2024, 2, 10, 12), at(2024, 3, 10, 12));
        let readings = noon.readings();
        assert_eq!(readings.months.to_string(), "1");
        assert_eq!(readings.days.to_string(), "29");
        assert_eq!(readings.project(Time::Day).unwrap().to_string(), "29");
    }

    #[test]
    fn epoch_seconds_round_trip_through_float() {
        let moment = at(2024, 2, 10, 12);
        let seconds = epoch_seconds(&moment);
        let restored = zoned_from_epoch_seconds(&seconds, &zone()).unwrap();
        assert_eq!(restored, moment);
    }

    #[test]
    fn epoch_seconds_round_trip_before_the_epoch() {
        let moment = at(1932, 7, 4, 9);
        let seconds = epoch_seconds(&moment);
        assert!(seconds < Float::from(0));
        let restored = zoned_from_epoch_seconds(&seconds, &zone()).unwrap();
        assert_eq!(restored, moment);
    }
}
