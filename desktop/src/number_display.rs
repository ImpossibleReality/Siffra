//! Fitting a result to the width the output column has for it.
//!
//! Full-precision values are shown as recurring decimals when possible;
//! otherwise they are rounded to the most precise form that fits, without
//! discarding integer digits or units.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NumberDisplay {
    pub before_repeat: String,
    pub repeating: String,
    pub after_repeat: String,
}

impl NumberDisplay {
    pub fn plain(value: impl Into<String>) -> Self {
        Self {
            before_repeat: value.into(),
            repeating: String::new(),
            after_repeat: String::new(),
        }
    }

    /// The character count the output column has to find room for.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.before_repeat.chars().count()
            + self.repeating.chars().count()
            + self.after_repeat.chars().count()
    }
}

/// A recurring suffix shorter than this reads as coincidence rather than as a
/// repetend, so it is left alone.
const MIN_REPEATING_DIGITS: usize = 12;

/// The parsed sign, grouped integer, fraction, exponent, and unit suffix.
struct Parts<'a> {
    sign: &'a str,
    integer: &'a str,
    fraction: Option<&'a str>,
    exponent: &'a str,
    suffix: &'a str,
}

fn parse(value: &str) -> Option<Parts<'_>> {
    let bytes = value.as_bytes();
    let sign_end = if matches!(bytes.first(), Some(b'+' | b'-')) {
        1
    } else {
        0
    };
    let mut cursor = sign_end;

    if !bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let integer_start = cursor;
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b',')
    {
        cursor += 1;
    }
    let integer_end = cursor;

    let mut fraction = None;
    if bytes.get(cursor) == Some(&b'.') {
        let digits_start = cursor + 1;
        let mut digits_end = digits_start;
        while bytes.get(digits_end).is_some_and(u8::is_ascii_digit) {
            digits_end += 1;
        }
        if digits_end > digits_start {
            fraction = Some(&value[digits_start..digits_end]);
            cursor = digits_end;
        }
    }

    let mut exponent = "";
    if bytes.get(cursor) == Some(&b'E') {
        let digits_start = if matches!(bytes.get(cursor + 1), Some(b'+' | b'-')) {
            cursor + 2
        } else {
            cursor + 1
        };
        let mut digits_end = digits_start;
        while bytes.get(digits_end).is_some_and(u8::is_ascii_digit) {
            digits_end += 1;
        }
        if digits_end > digits_start {
            exponent = &value[cursor..digits_end];
            cursor = digits_end;
        }
    }

    Some(Parts {
        sign: &value[..sign_end],
        integer: &value[integer_start..integer_end],
        fraction,
        exponent,
        suffix: &value[cursor..],
    })
}

/// Finds a recurring decimal suffix with a period of one, two, or three digits.
/// Requiring a long suffix keeps ordinary values that happen to end in a few
/// equal digits from being written as recurring ones.
pub fn format_repeating_decimal(value: &str) -> Option<NumberDisplay> {
    let parts = parse(value)?;
    let fraction = parts.fraction.unwrap_or("");
    if fraction.len() < MIN_REPEATING_DIGITS {
        return None;
    }

    let digits = fraction.as_bytes();
    for start in 0..fraction.len() {
        for period in 1..=3usize {
            let repeated_len = fraction.len() - start;
            if repeated_len < MIN_REPEATING_DIGITS
                || repeated_len < period * 4
                || start + period > fraction.len()
            {
                continue;
            }

            let repetend = &digits[start..start + period];
            if (start..fraction.len()).all(|i| digits[i] == repetend[(i - start) % period]) {
                return Some(NumberDisplay {
                    before_repeat: format!(
                        "{}{}.{}",
                        parts.sign,
                        parts.integer,
                        &fraction[..start]
                    ),
                    repeating: fraction[start..start + period].to_string(),
                    after_repeat: format!("{}{}", parts.exponent, parts.suffix),
                });
            }
        }
    }

    None
}

fn group_integer(integer: &str) -> String {
    let mut out = String::with_capacity(integer.len() + integer.len() / 3);
    let digits = integer.as_bytes();
    for (i, digit) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*digit as char);
    }
    out
}

fn increment(digits: &str) -> String {
    let mut out = digits.as_bytes().to_vec();
    for byte in out.iter_mut().rev() {
        if *byte == b'9' {
            *byte = b'0';
        } else {
            *byte += 1;
            return String::from_utf8(out).expect("digits stay ASCII");
        }
    }
    let mut carried = String::from("1");
    carried.push_str(&String::from_utf8(out).expect("digits stay ASCII"));
    carried
}

/// Rounds `value` to `places` decimals, half away from zero, keeping its sign,
/// exponent and unit. Returns `value` unchanged when it is already that short.
fn round_to_decimal_places(value: &str, places: usize) -> Option<String> {
    let parts = parse(value)?;
    let fraction = parts.fraction?;
    if places >= fraction.len() {
        return Some(value.to_string());
    }

    let integer: String = parts.integer.chars().filter(|c| *c != ',').collect();
    let mut scaled = format!("{integer}{}", &fraction[..places]);
    scaled = scaled.trim_start_matches('0').to_string();
    if scaled.is_empty() {
        scaled.push('0');
    }
    if fraction.as_bytes()[places] >= b'5' {
        scaled = increment(&scaled);
    }

    let digits = if scaled.len() < places + 1 {
        format!("{}{scaled}", "0".repeat(places + 1 - scaled.len()))
    } else {
        scaled
    };

    let (rounded_integer, rounded_fraction) = if places == 0 {
        (digits.as_str(), "")
    } else {
        let split = digits.len() - places;
        (&digits[..split], digits[split..].trim_end_matches('0'))
    };

    let decimal = if rounded_fraction.is_empty() {
        String::new()
    } else {
        format!(".{rounded_fraction}")
    };

    Some(format!(
        "{}{}{decimal}{}{}",
        parts.sign,
        group_integer(rounded_integer),
        parts.exponent,
        parts.suffix
    ))
}

/// The most precise representation of `value` that fits `max_characters` of the
/// monospaced column.
pub fn format_number_for_characters(
    value: &str,
    max_characters: usize,
    repeating: bool,
) -> NumberDisplay {
    if repeating
        && let Some(display) = format_repeating_decimal(value)
        && display.len() <= max_characters
    {
        return display;
    }

    if value.chars().count() <= max_characters {
        return NumberDisplay::plain(value);
    }

    let Some(fraction_len) = parse(value).and_then(|parts| parts.fraction).map(str::len) else {
        return NumberDisplay::plain(value);
    };

    for places in (0..fraction_len).rev() {
        if let Some(candidate) = round_to_decimal_places(value, places)
            && candidate.chars().count() <= max_characters
        {
            return NumberDisplay::plain(candidate);
        }
    }

    // Integer digits and units are never discarded. The column clips whatever is
    // left over when it is too narrow even for that.
    NumberDisplay::plain(round_to_decimal_places(value, 0).unwrap_or_else(|| value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_recurring_decimal_with_its_repetend_marked() {
        let display = format_repeating_decimal("0.3333333333333").unwrap();
        assert_eq!(display.before_repeat, "0.");
        assert_eq!(display.repeating, "3");
        assert_eq!(display.after_repeat, "");
    }

    #[test]
    fn keeps_a_recurring_decimals_lead_in_digits() {
        let display = format_repeating_decimal("1.1666666666666 kg").unwrap();
        assert_eq!(display.before_repeat, "1.1");
        assert_eq!(display.repeating, "6");
        assert_eq!(display.after_repeat, " kg");
    }

    #[test]
    fn leaves_short_fractions_alone() {
        assert_eq!(format_repeating_decimal("0.333"), None);
    }

    #[test]
    fn rounds_to_the_widest_precision_that_fits() {
        assert_eq!(
            format_number_for_characters("3.14159265358979", 6, false),
            NumberDisplay::plain("3.1416")
        );
    }

    #[test]
    fn rounding_carries_into_the_integer_digits() {
        assert_eq!(
            format_number_for_characters("9.99", 3, false),
            NumberDisplay::plain("10")
        );
    }

    #[test]
    fn keeps_the_unit_and_regroups_the_integer_digits() {
        assert_eq!(
            format_number_for_characters("1234567.891 m", 12, false),
            NumberDisplay::plain("1,234,568 m")
        );
    }

    #[test]
    fn leaves_a_value_that_already_fits_exactly_as_it_came() {
        assert_eq!(
            format_number_for_characters("1234567.891 m", 13, false),
            NumberDisplay::plain("1234567.891 m")
        );
    }

    #[test]
    fn never_discards_integer_digits_however_narrow_the_column() {
        assert_eq!(
            format_number_for_characters("1234.5678 kg", 2, false),
            NumberDisplay::plain("1,235 kg")
        );
    }

    #[test]
    fn passes_through_values_that_already_fit() {
        assert_eq!(
            format_number_for_characters("42 mol", 20, false),
            NumberDisplay::plain("42 mol")
        );
    }

    #[test]
    fn prefers_the_repetend_whenever_it_fits() {
        let display = format_number_for_characters("0.6666666666666", 5, true);
        assert_eq!(display.before_repeat, "0.");
        assert_eq!(display.repeating, "6");
    }

    #[test]
    fn falls_back_to_rounding_when_the_repetend_does_not_fit() {
        assert_eq!(
            format_number_for_characters("0.6666666666666", 2, true),
            NumberDisplay::plain("1")
        );
    }

    #[test]
    fn groups_integers_in_threes() {
        assert_eq!(group_integer("1"), "1");
        assert_eq!(group_integer("1234"), "1,234");
        assert_eq!(group_integer("1234567"), "1,234,567");
    }

    #[test]
    fn increments_across_a_full_carry() {
        assert_eq!(increment("999"), "1000");
        assert_eq!(increment("1239"), "1240");
    }
}
