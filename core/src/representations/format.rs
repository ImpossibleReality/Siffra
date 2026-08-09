use crate::representations::basis::zoned_from_epoch_seconds;
use crate::representations::{Affine, Basis, Float, Granularity, TimeZone, Value};

/// A rendered value, still structured. The front end applies its own
/// character-budget rules per arm instead of re-parsing a flat string, which
/// decimal-only assumptions would corrupt for instants and reading lists.
#[derive(Debug, Clone, PartialEq)]
pub enum FormattedValue {
    Number {
        text: String,
        unit: Option<String>,
    },
    /// A date, datetime or wall clock, already rendered at its granularity,
    /// zone suffix included when it is not the document default.
    Instant {
        text: String,
    },
    /// One entry per calendar granularity that is exact and nonzero.
    SpanReadings {
        readings: Vec<(String, String)>,
    },
}

impl FormattedValue {
    /// A flat rendering, for contexts that cannot use the structure.
    pub fn to_plain_string(&self) -> String {
        match self {
            FormattedValue::Number { text, unit } => match unit {
                Some(unit) => format!("{text} {unit}"),
                None => text.clone(),
            },
            FormattedValue::Instant { text } => text.clone(),
            FormattedValue::SpanReadings { readings } => readings
                .iter()
                .map(|(amount, unit)| format!("{amount} {unit}"))
                .collect::<Vec<_>>()
                .join(" / "),
        }
    }
}

/// Renders `value`. `default_zone` suppresses the zone suffix on instants that
/// are already read in the document's zone, mirroring currency shorthand.
pub fn format_value(value: &Value, default_zone: Option<&TimeZone>) -> FormattedValue {
    if let Some(point) = value.dimension.as_temporal_point() {
        return format_temporal(value, &point, default_zone);
    }
    if let Some(basis) = value.basis()
        && value.display().span_readings
    {
        return FormattedValue::SpanReadings {
            readings: span_readings(basis),
        };
    }

    let unit = value.dimension.to_string();
    FormattedValue::Number {
        text: value.value.to_string(),
        unit: (!unit.is_empty()).then_some(unit),
    }
}

fn format_temporal(
    value: &Value,
    point: &Affine,
    default_zone: Option<&TimeZone>,
) -> FormattedValue {
    let zone = point.zone().expect("a temporal point names a zone");
    let Some(moment) = zoned_from_epoch_seconds(&value.value, zone) else {
        return FormattedValue::Number {
            text: value.value.to_string(),
            unit: Some(zone.label().to_string()),
        };
    };

    let granularity = value.display().granularity.unwrap_or(if point.is_clock() {
        Granularity::Minute
    } else {
        Granularity::Second
    });
    let date = moment.date();
    let time = moment.time();
    // TODO: Internationalize date order and 12/24-hour time preferences.
    let date = format!("{:02}-{:02}-{:04}", date.month(), date.day(), date.year());
    let clock = |with_seconds| {
        let hour = match time.hour() % 12 {
            0 => 12,
            hour => hour,
        };
        let meridiem = if time.hour() < 12 { "AM" } else { "PM" };
        if with_seconds {
            format!(
                "{hour}:{:02}:{:02} {meridiem}",
                time.minute(),
                time.second()
            )
        } else {
            format!("{hour}:{:02} {meridiem}", time.minute())
        }
    };
    // A clock's date is only there to resolve the zone's offset, so it is
    // never shown, not even when a shift has carried the reading into the
    // next day.
    let mut text = match (point.is_clock(), granularity) {
        (true, Granularity::Second) => clock(true),
        (true, _) => clock(false),
        (false, Granularity::Date) => date,
        (false, Granularity::Minute) => format!("{date} {}", clock(false)),
        (false, Granularity::Second) => format!("{date} {}", clock(true)),
    };

    if default_zone.is_none_or(|default| default != zone) {
        text.push(' ');
        text.push_str(zone.label());
    }
    FormattedValue::Instant { text }
}

const HOUR_SECONDS: i64 = 3600;
const READING_DECIMALS: i32 = 4;

/// Months, days and hours are the readings the calendar axes disagree about,
/// so they are what a span shows by default; years join in once there is one.
fn span_readings(basis: &Basis) -> Vec<(String, String)> {
    let readings = basis.readings();
    let hours = &readings.seconds / &Float::from(HOUR_SECONDS);

    let mut out = Vec::new();
    let mut push = |amount: Float, singular: &str, plural: &str| {
        // Calendar totals come from f64, and elapsed hours often recur; neither
        // deserves thirty digits in a summary line.
        let amount = amount.round_decimal_places(READING_DECIMALS);
        if amount == Float::from(0) {
            return;
        }
        let label = if amount == Float::from(1) || amount == Float::from(-1) {
            singular
        } else {
            plural
        };
        out.push((amount.to_string(), label.to_string()));
    };

    if readings.years.abs() >= Float::from(1) {
        push(readings.years.clone(), "year", "years");
    }
    push(readings.months.clone(), "month", "months");
    push(readings.days.clone(), "day", "days");
    push(hours, "hour", "hours");
    if out.is_empty() {
        out.push(("0".to_string(), "hours".to_string()));
    }
    out
}
