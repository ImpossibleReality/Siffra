//! Mixed-unit display: splitting one scalar across several units.
//!
//! A ladder is an ordered list of same-dimensionality units, largest to
//! smallest, whose rungs **sum** to the value: `5 ft 2 in` is one length.
//! The value itself stays canonical — the ladder is only a display hint
//! and dropping it is OK.

use crate::representations::{Basis, Dimension, Float, Quantity, Time, Unit, is_calendar_unit};
use jiff::{Span as JiffSpan, Unit as JiffUnit, Zoned};

const LADDER_DECIMALS: i32 = 3;

/// Why a list of units cannot be read as a ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderError {
    Empty,
    MixedDimensions,
    NotDecreasing,
    NotAdditive,
    NotFixed,
}

impl LadderError {
    pub fn message(self) -> &'static str {
        match self {
            LadderError::Empty => "A ladder needs at least one unit",
            LadderError::MixedDimensions => "Ladder units must all measure the same thing",
            LadderError::NotDecreasing => "Ladder units must go from largest to smallest",
            LadderError::NotAdditive => "Ladder units must be amounts, not points on a scale",
            LadderError::NotFixed => {
                "Ladder units must be a fixed size, so currencies cannot be rungs"
            }
        }
    }
}

/// One rung of a ladder as a single-unit dimension, which is how the rest of
/// the engine talks about units.
pub fn rung_dimension(unit: &Unit) -> Dimension {
    Dimension(vec![(unit.clone(), Float::from(1))])
}

/// Rungs are summed, so each one has to be a plain amount of a size that does
/// not move. An affine scale carries an offset, which makes `55 °C + 1 °F` no
/// temperature at all, and a currency's size is whatever the market says today,
/// which would leave the same ladder valid one day and rejected the next.
fn eligible(unit: &Unit) -> Result<(), LadderError> {
    match unit {
        Unit::Prefixed(_, inner) => eligible(inner),
        Unit::Absolute(_) | Unit::Instant(_) | Unit::Clock(_) => Err(LadderError::NotAdditive),
        Unit::Currency(_) => Err(LadderError::NotFixed),
        _ => Ok(()),
    }
}

/// Rungs have to measure one thing and get strictly smaller
pub fn validate(units: &[Unit]) -> Result<(), LadderError> {
    let Some(first) = units.first() else {
        return Err(LadderError::Empty);
    };
    let dimensionality = first.dimensionality();
    let mut previous: Option<Float> = None;

    for unit in units {
        eligible(unit)?;
        if unit.dimensionality() != dimensionality {
            return Err(LadderError::MixedDimensions);
        }
        let factor = unit.factor().ok_or(LadderError::MixedDimensions)?;
        if let Some(previous) = &previous
            && *previous <= factor
        {
            return Err(LadderError::NotDecreasing);
        }
        previous = Some(factor);
    }
    Ok(())
}

/// Whether any rung is a civil unit whose length depends on the calendar
pub fn has_calendar_rung(units: &[Unit]) -> bool {
    units.iter().any(|unit| calendar_time(unit).is_some())
}

fn calendar_time(unit: &Unit) -> Option<Time> {
    match unit {
        Unit::Base(Quantity::Time(time)) if is_calendar_unit(*time) => Some(*time),
        _ => None,
    }
}

fn time_unit(unit: &Unit) -> Option<Time> {
    match unit {
        Unit::Base(Quantity::Time(time)) => Some(*time),
        _ => None,
    }
}

/// A rung as jiff can count it. Fortnights, decades, centuries and millennia
/// have no unit of their own there, but each is a whole number of one that
/// does, so they are counted in that one and divided back down.
#[derive(Clone, Copy)]
struct Rung {
    time: Time,
    jiff: JiffUnit,
    per_rung: i64,
}

fn rung(unit: &Unit) -> Option<Rung> {
    let (time, jiff, per_rung) = match time_unit(unit)? {
        Time::Second => (Time::Second, JiffUnit::Second, 1),
        Time::Minute => (Time::Minute, JiffUnit::Minute, 1),
        Time::Hour => (Time::Hour, JiffUnit::Hour, 1),
        Time::Day => (Time::Day, JiffUnit::Day, 1),
        Time::Week => (Time::Week, JiffUnit::Week, 1),
        Time::Fortnight => (Time::Week, JiffUnit::Week, 2),
        Time::Month => (Time::Month, JiffUnit::Month, 1),
        Time::Year => (Time::Year, JiffUnit::Year, 1),
        Time::Decade => (Time::Year, JiffUnit::Year, 10),
        Time::Century => (Time::Year, JiffUnit::Year, 100),
        Time::Millennium => (Time::Year, JiffUnit::Year, 1_000),
    };
    Some(Rung {
        time,
        jiff,
        per_rung,
    })
}

/// A decomposed value: the sign, then one amount per rung the ladder names.
#[derive(Debug, Clone, PartialEq)]
pub struct Decomposition {
    pub negative: bool,
    pub rungs: Vec<(Float, Unit)>,
}

impl Decomposition {
    /// Amount and unit label per rung, ready to render. Zero rungs are kept:
    /// the ladder was asked for by name, so it answers by name.
    pub fn rendered(&self) -> Vec<(String, String)> {
        self.rungs
            .iter()
            .map(|(amount, unit)| (render_amount(amount), unit.shorthand()))
            .collect()
    }

    /// Whether the leading sign needs a bracket to reach the whole value.
    /// `-0 ft 10 in` reads as ten positive inches to anyone who stops at the
    /// first rung; `-(0 ft 10 in)` cannot be read that way.
    pub fn grouped(&self) -> bool {
        self.negative
            && self
                .rungs
                .first()
                .is_some_and(|(amount, _)| *amount == Float::from(0))
    }
}

/// A whole zero prints as `0`, not as the `0.0` a bare `Float` would give.
fn render_amount(amount: &Float) -> String {
    if *amount == Float::from(0) {
        "0".to_string()
    } else {
        amount.to_string()
    }
}

/// Splits `value` (measured in `dimension`) across `units`.
///
/// Endpoints are used whenever the ladder crosses an irregular rung, because
/// only they know how long a civil day or a month actually was. Everything else
/// is exact ratios on the scalar. Returns `None` when the units cannot be
/// compared at all, so the caller can fall back to a plain number.
pub fn decompose(
    value: &Float,
    dimension: &Dimension,
    basis: Option<&Basis>,
    units: &[Unit],
) -> Option<Decomposition> {
    if validate(units).is_err() {
        return None;
    }
    if has_calendar_rung(units)
        && let Some((start, end)) = basis.and_then(Basis::anchors)
        && let Some(decomposition) = anchored(start, end, units)
    {
        return Some(decomposition);
    }
    exact(value, dimension, units)
}

/// Splits an unanchored duration into the exact rungs used to shift a calendar.
/// Unlike display decomposition, the last rung is not rounded: these amounts
/// participate in evaluation rather than presentation.
pub fn shift_rungs(
    value: &Float,
    dimension: &Dimension,
    basis: Option<&Basis>,
    units: &[Unit],
) -> Option<Vec<(Float, Unit)>> {
    if validate(units).is_err() {
        return None;
    }

    if let Some((start, end)) = basis.and_then(Basis::anchors)
        && let Some(decomposition) = anchored(start, end, units)
    {
        return Some(signed_rungs(decomposition));
    }

    let factors = units
        .iter()
        .map(Unit::factor)
        .collect::<Option<Vec<Float>>>()?;
    let negative = *value < Float::from(0);
    let mut remaining = (value * &dimension.factor()?).abs();
    let mut amounts = Vec::with_capacity(units.len());

    for factor in &factors[..factors.len() - 1] {
        let whole = (&remaining / factor).floor();
        remaining = &remaining - &(&whole * factor);
        amounts.push(whole);
    }
    amounts.push(&remaining / factors.last().expect("a validated ladder has rungs"));

    Some(signed_rungs(Decomposition {
        negative,
        rungs: amounts.into_iter().zip(units.iter().cloned()).collect(),
    }))
}

fn signed_rungs(decomposition: Decomposition) -> Vec<(Float, Unit)> {
    decomposition
        .rungs
        .into_iter()
        .map(|(amount, unit)| {
            let amount = if decomposition.negative {
                -amount
            } else {
                amount
            };
            (amount, unit)
        })
        .collect()
}

/// Ratios alone: a day is 86,400 s and a month is its mean length, which is
/// what a duration without endpoints means.
fn exact(value: &Float, dimension: &Dimension, units: &[Unit]) -> Option<Decomposition> {
    let factors = units
        .iter()
        .map(Unit::factor)
        .collect::<Option<Vec<Float>>>()?;
    let mut remaining = (value * &dimension.factor()?).abs();

    let mut amounts = Vec::with_capacity(units.len());
    for factor in &factors[..factors.len() - 1] {
        let whole = (&remaining / factor).floor();
        remaining = &remaining - &(&whole * factor);
        amounts.push(whole);
    }
    let last = factors.last().expect("a ladder has at least one rung");
    let final_rung = round_last(&(&remaining / last), &amounts);
    amounts.push(final_rung);
    carry(&mut amounts, &factors);

    Some(Decomposition {
        negative: *value < Float::from(0),
        rungs: amounts.into_iter().zip(units.iter().cloned()).collect(),
    })
}

/// Rounds the smallest rung.
fn round_last(amount: &Float, above: &[Float]) -> Float {
    let zero = Float::from(0);
    let rounded = amount.round_decimal_places(LADDER_DECIMALS);
    if rounded == zero && *amount != zero && above.iter().all(|amount| *amount == zero) {
        return amount.round_significant_digits(1);
    }
    rounded
}

/// Rebalances after rounding, so `1 ft 11.9999… in` reads `2 ft 0 in` rather
/// than the `1 ft 12 in` that no one writes. This also absorbs the occasional
/// binary-precision crumb left by flooring an exact ratio.
fn carry(amounts: &mut [Float], factors: &[Float]) {
    for index in (1..amounts.len()).rev() {
        let per_rung = &factors[index - 1] / &factors[index];
        if amounts[index] < per_rung {
            continue;
        }
        let carried = (&amounts[index] / &per_rung).floor();
        amounts[index] =
            (&amounts[index] - &(&carried * &per_rung)).round_decimal_places(LADDER_DECIMALS);
        amounts[index - 1] = &amounts[index - 1] + &carried;
    }
}

/// Walks the ladder from the real start date, letting jiff count whole civil
/// units at each rung. E.x. a noon-to-noon February stretch reads
/// as 29 days even though only 695 hours elapsed.
fn anchored(start: &Zoned, end: &Zoned, units: &[Unit]) -> Option<Decomposition> {
    let rungs = units.iter().map(rung).collect::<Option<Vec<_>>>()?;
    let negative = end < start;
    let (start, end) = if negative { (end, start) } else { (start, end) };
    // A span is counted forward from where it started, and jiff will not count
    // calendar units across two zones at all, so the far end is read on the
    // near end's calendar. Only the wall reading moves; the instant does not.
    let end = &end.clone().with_time_zone(start.time_zone().clone());

    let mut cursor = start.clone();
    let mut amounts = Vec::with_capacity(units.len());
    for rung in rungs.iter().take(rungs.len() - 1) {
        let span = cursor.until((rung.jiff, end)).ok()?;
        let whole = whole_field(&span, rung.time) / rung.per_rung;
        let counted = whole.checked_mul(rung.per_rung)?;
        cursor = cursor.checked_add(span_of(rung.time, counted)?).ok()?;
        amounts.push(Float::from(whole));
    }

    let last = *rungs.last().expect("a ladder has at least one rung");
    // An earlier rung can land exactly on the end. Jiff cannot total a zero
    // span in a calendar unit — it asserts internally — and zero is the answer
    // anyway, so it is written directly.
    let remainder = if cursor.timestamp() == end.timestamp() {
        Float::from(0)
    } else {
        let total = cursor.until(end).ok()?.total((last.jiff, &cursor)).ok()?;
        &float_from_f64(total)? / &Float::from(last.per_rung)
    };
    let final_rung = round_last(&remainder, &amounts);
    amounts.push(final_rung);

    Some(Decomposition {
        negative,
        rungs: amounts.into_iter().zip(units.iter().cloned()).collect(),
    })
}

/// The whole count jiff put on the rung's own field. A span balanced with this
/// unit as its largest keeps everything smaller in the fields below it.
fn whole_field(span: &JiffSpan, unit: Time) -> i64 {
    match unit {
        Time::Second => span.get_seconds(),
        Time::Minute => span.get_minutes(),
        Time::Hour => i64::from(span.get_hours()),
        Time::Day => i64::from(span.get_days()),
        Time::Week => i64::from(span.get_weeks()),
        Time::Month => i64::from(span.get_months()),
        Time::Year => i64::from(span.get_years()),
        _ => 0,
    }
}

fn span_of(unit: Time, amount: i64) -> Option<JiffSpan> {
    let span = JiffSpan::new();
    match unit {
        Time::Second => span.try_seconds(amount),
        Time::Minute => span.try_minutes(amount),
        Time::Hour => span.try_hours(amount),
        Time::Day => span.try_days(amount),
        Time::Week => span.try_weeks(amount),
        Time::Month => span.try_months(amount),
        Time::Year => span.try_years(amount),
        _ => return None,
    }
    .ok()
}

fn float_from_f64(value: f64) -> Option<Float> {
    if !value.is_finite() {
        return None;
    }
    Float::parse(&value.to_string()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::representations::{Length, Time};
    use jiff::civil::date;

    fn unit(name: &str) -> Unit {
        Unit::from_str(name).expect("a known unit")
    }

    fn rendered(value: &str, dimension: &str, rungs: &[&str]) -> String {
        let units: Vec<Unit> = rungs.iter().map(|name| unit(name)).collect();
        let decomposition = decompose(
            &Float::parse(value).unwrap(),
            &rung_dimension(&unit(dimension)),
            None,
            &units,
        )
        .expect("a comparable ladder");
        let text = decomposition
            .rendered()
            .into_iter()
            .map(|(amount, unit)| format!("{amount} {unit}"))
            .collect::<Vec<_>>()
            .join(" ");
        if decomposition.negative {
            format!("-{text}")
        } else {
            text
        }
    }

    #[test]
    fn rungs_take_whole_amounts_and_the_last_one_keeps_the_fraction() {
        assert_eq!(rendered("62", "in", &["ft", "in"]), "5 ft 2 in");
        assert_eq!(rendered("100.5", "in", &["ft", "in"]), "8 ft 4.5 in");
        assert_eq!(rendered("7502", "s", &["h", "min", "s"]), "2 h 5 min 2 s");
    }

    #[test]
    fn every_named_rung_shows_even_at_zero() {
        assert_eq!(rendered("0.5", "d", &["h", "min"]), "12 h 0 min");
    }

    #[test]
    fn a_rounded_last_rung_carries_into_the_one_above_it() {
        // 1 ft 11.99999 in is 2 ft, and nobody writes twelve inches.
        assert_eq!(rendered("23.99999", "in", &["ft", "in"]), "2 ft 0 in");
    }

    #[test]
    fn the_sign_is_written_once_in_front() {
        assert_eq!(rendered("-62", "in", &["ft", "in"]), "-5 ft 2 in");
    }

    #[test]
    fn a_ladder_must_be_one_dimension_and_strictly_decreasing() {
        let length = |names: &[&str]| names.iter().map(|name| unit(name)).collect::<Vec<_>>();

        assert_eq!(validate(&length(&["ft", "in"])), Ok(()));
        assert_eq!(
            validate(&length(&["in", "ft"])),
            Err(LadderError::NotDecreasing)
        );
        assert_eq!(
            validate(&length(&["ft", "ft"])),
            Err(LadderError::NotDecreasing)
        );
        assert_eq!(
            validate(&length(&["m", "kg"])),
            Err(LadderError::MixedDimensions)
        );
        assert_eq!(validate(&[]), Err(LadderError::Empty));
    }

    #[test]
    fn a_rung_has_to_be_an_amount_of_a_size_that_holds_still() {
        let zone = crate::representations::TimeZone::parse("UTC").unwrap();
        for rungs in [
            vec![unit("C"), unit("F")],
            vec![Unit::Instant(zone.clone()), unit("h")],
            vec![Unit::Clock(zone), unit("h")],
        ] {
            assert_eq!(validate(&rungs), Err(LadderError::NotAdditive));
        }
        assert_eq!(
            validate(&[unit("USD"), unit("EUR")]),
            Err(LadderError::NotFixed)
        );
    }

    #[test]
    fn anchored_rungs_count_civil_days_not_multiples_of_86400() {
        let zone = crate::representations::TimeZone::parse("America/Los_Angeles").unwrap();
        let at = |month: i8, day: i8, hour: i8| {
            date(2024, month, day)
                .at(hour, 0, 0, 0)
                .to_zoned(zone.inner().clone())
                .expect("a real wall clock time")
        };
        // 695 hours elapsed across the spring-forward, but 29 civil days.
        let basis = Basis::span(at(2, 10, 12), at(3, 10, 12));
        let decomposition = decompose(
            &Float::from(695),
            &rung_dimension(&Unit::Base(Quantity::Time(Time::Hour))),
            Some(&basis),
            &[unit("d"), unit("h")],
        )
        .expect("an anchored ladder");

        assert_eq!(
            decomposition.rendered(),
            vec![
                ("29".to_string(), "d".to_string()),
                ("0".to_string(), "h".to_string())
            ]
        );
    }

    #[test]
    fn a_ladder_of_unknown_units_declines_rather_than_guessing() {
        assert_eq!(
            decompose(
                &Float::from(1),
                &rung_dimension(&Unit::Base(Quantity::Length(Length::Meter))),
                None,
                &[]
            ),
            None
        );
    }
}
