use jiff::tz;

const ABBREVIATION_OFFSETS: &[(&str, i32)] = &[
    ("PST", -8),
    ("PDT", -7),
    ("MST", -7),
    ("MDT", -6),
    ("CST", -6),
    ("CDT", -5),
    ("EST", -5),
    ("EDT", -4),
    ("AKST", -9),
    ("AKDT", -8),
    ("HST", -10),
];

/// The timezone an instant is read in. Kept in the unit because calendar
/// arithmetic consults it, not just rendering.
///
/// Equality is by name so that `Dimension`'s structural `PartialEq` keeps
/// working; two handles to the same zone always carry the same label.
#[derive(Debug, Clone)]
pub struct TimeZone {
    label: String,
    inner: tz::TimeZone,
}

impl PartialEq for TimeZone {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

impl TimeZone {
    pub fn utc() -> Self {
        Self {
            label: "UTC".to_string(),
            inner: tz::TimeZone::UTC,
        }
    }

    /// The machine's zone, falling back to UTC when it cannot be determined.
    pub fn system() -> Self {
        let inner = tz::TimeZone::system();
        match inner.iana_name() {
            Some(name) => Self {
                label: name.to_string(),
                inner,
            },
            None => Self::utc(),
        }
    }

    /// Accepts IANA names (`Europe/Stockholm`), `UTC`, fixed offsets
    /// (`+05:30`), and hardcoded North American abbreviations such as `PST`.
    /// Abbreviations are fixed offsets and do not apply daylight-saving
    /// transitions automatically.
    pub fn parse(text: &str) -> Option<Self> {
        if matches!(text, "UTC" | "utc" | "GMT" | "gmt") {
            return Some(Self::utc());
        }
        if let Some((label, hours)) = ABBREVIATION_OFFSETS
            .iter()
            .find(|(label, _)| *label == text)
        {
            return Some(Self {
                label: (*label).to_string(),
                inner: tz::TimeZone::fixed(
                    tz::Offset::from_seconds(hours * 3600).expect("valid abbreviation offset"),
                ),
            });
        }
        if let Some(offset) = parse_offset(text) {
            return Some(Self {
                label: text.to_string(),
                inner: tz::TimeZone::fixed(offset),
            });
        }
        if !is_iana_shaped(text) {
            return None;
        }
        let inner = tz::TimeZone::get(text).ok()?;
        Some(Self {
            label: inner.iana_name().unwrap_or(text).to_string(),
            inner,
        })
    }

    pub(crate) fn abbreviations() -> impl Iterator<Item = &'static str> {
        ABBREVIATION_OFFSETS.iter().map(|(name, _)| *name)
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn inner(&self) -> &tz::TimeZone {
        &self.inner
    }
}

/// IANA names are `Area/Location`, always starting with an uppercase letter.
/// Requiring the shape up front keeps `5 m in m/s` from reaching a tzdb lookup.
fn is_iana_shaped(text: &str) -> bool {
    let mut segments = text.split('/');
    let Some(area) = segments.next() else {
        return false;
    };
    if segments.clone().count() == 0 {
        return false;
    }
    let segment_ok = |segment: &str| {
        segment
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+'))
    };
    segment_ok(area) && segments.all(segment_ok)
}

fn parse_offset(text: &str) -> Option<tz::Offset> {
    let (sign, rest) = match text.as_bytes().first()? {
        b'+' => (1i8, &text[1..]),
        b'-' => (-1i8, &text[1..]),
        _ => return None,
    };
    let (hours, minutes) = match rest.split_once(':') {
        Some((hours, minutes)) => (hours, minutes.parse::<i32>().ok()?),
        None => (rest, 0),
    };
    if hours.is_empty() || !hours.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let seconds = hours.parse::<i32>().ok()? * 3600 + minutes * 60;
    tz::Offset::from_seconds(seconds * sign as i32).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_zone_spellings_without_claiming_unit_names() {
        assert_eq!(
            TimeZone::parse("Europe/Stockholm").unwrap().label(),
            "Europe/Stockholm"
        );
        assert_eq!(TimeZone::parse("UTC").unwrap().label(), "UTC");
        assert!(TimeZone::parse("+05:30").is_some());
        assert!(TimeZone::parse("-08").is_some());

        assert!(TimeZone::parse("m/s").is_none());
        assert!(TimeZone::parse("kg").is_none());
        assert_eq!(TimeZone::parse("PST").unwrap().label(), "PST");
        assert_eq!(
            TimeZone::parse("PDT")
                .unwrap()
                .inner()
                .to_offset(jiff::Timestamp::UNIX_EPOCH)
                .seconds(),
            -7 * 3600
        );
        assert!(TimeZone::parse("Europe/Nowhere").is_none());
    }
}
