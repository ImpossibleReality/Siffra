use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::representations::basis::{epoch_seconds, zoned_from_epoch_seconds};
pub use crate::representations::*;
use jiff::{Span as JiffSpan, Zoned};

/// A scalar with a dimension and associated metadata
#[derive(Debug, Clone)]
pub struct Value {
    pub dimension: Dimension,
    pub value: Float,
    basis: Option<Basis>,
    display: DisplayHints,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.dimension == other.dimension && self.value == other.value
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::new(Float::from(value), None)
    }
}

fn seconds_dimension() -> Dimension {
    Dimension(vec![(
        Unit::Base(Quantity::Time(Time::Second)),
        Float::from(1),
    )])
}

fn instant_dimension(zone: &TimeZone) -> Dimension {
    Dimension(vec![(Unit::Instant(zone.clone()), Float::from(1))])
}

fn clock_dimension(zone: &TimeZone) -> Dimension {
    Dimension(vec![(Unit::Clock(zone.clone()), Float::from(1))])
}

fn hours_dimension() -> Dimension {
    Dimension(vec![(
        Unit::Base(Quantity::Time(Time::Hour)),
        Float::from(1),
    )])
}

/// The dimension a temporal point keeps when it moves: shifting a clock leaves
/// a clock, shifting an instant leaves an instant.
fn point_dimension(point: &Affine, zone: &TimeZone) -> Dimension {
    if point.is_clock() {
        clock_dimension(zone)
    } else {
        instant_dimension(zone)
    }
}

/// How much of an instant stays meaningful after adding a duration of this
/// dimension: whole days keep a bare date, hours and minutes need a clock.
fn duration_granularity(dimension: &Dimension) -> Granularity {
    let mut granularity = Granularity::Date;
    for (unit, _) in dimension.0.iter() {
        let unit = match unit {
            Unit::Prefixed(_, inner) => inner.as_ref(),
            other => other,
        };
        let Unit::Base(Quantity::Time(time)) = unit else {
            continue;
        };
        granularity = granularity.finer(match time {
            Time::Second => Granularity::Second,
            Time::Minute | Time::Hour => Granularity::Minute,
            _ => Granularity::Date,
        });
    }
    granularity
}

/// A jiff span for a whole number of calendar units, or `None` when the
/// duration is not one (fractional amounts, hours and below, compound units).
/// Callers fall back to exact seconds, which is degraded but still correct.
fn calendar_span(duration: &Value) -> Option<JiffSpan> {
    if duration.basis.is_some() {
        return None;
    }
    let unit = duration.dimension.as_calendar_unit()?;
    if !duration.value.is_integer() {
        return None;
    }
    let amount = duration.value.to_i64()?;
    let span = JiffSpan::new();
    match unit {
        Time::Day => span.try_days(amount),
        Time::Week => span.try_weeks(amount),
        Time::Fortnight => span.try_weeks(amount.checked_mul(2)?),
        Time::Month => span.try_months(amount),
        Time::Year => span.try_years(amount),
        Time::Decade => span.try_years(amount.checked_mul(10)?),
        Time::Century => span.try_years(amount.checked_mul(100)?),
        Time::Millennium => span.try_years(amount.checked_mul(1000)?),
        _ => return None,
    }
    .ok()
}

impl Value {
    pub fn new(value: Float, dimension: Option<Dimension>) -> Self {
        Self {
            dimension: dimension.unwrap_or_default(),
            value,
            basis: None,
            display: DisplayHints::default(),
        }
    }

    /// An instant: epoch seconds tagged with the zone it is read in.
    pub fn instant(moment: &Zoned, zone: &TimeZone, granularity: Granularity) -> Self {
        Self {
            dimension: instant_dimension(zone),
            value: epoch_seconds(moment),
            basis: None,
            display: DisplayHints::instant(granularity),
        }
    }

    /// A wall-clock reading. `moment` carries the date the zone's offset is
    /// resolved against, which the value keeps but never shows.
    pub fn clock(moment: &Zoned, zone: &TimeZone, granularity: Granularity) -> Self {
        Self {
            dimension: clock_dimension(zone),
            value: epoch_seconds(moment),
            basis: None,
            display: DisplayHints::instant(granularity),
        }
    }

    fn temporal_point(point: &Affine, moment: &Zoned, granularity: Granularity) -> Self {
        let zone = point.zone().expect("a temporal point names a zone");
        if point.is_clock() {
            Self::clock(moment, zone, granularity)
        } else {
            Self::instant(moment, zone, granularity)
        }
    }

    pub fn into_parts(self) -> (Float, Dimension) {
        (self.value, self.dimension)
    }

    pub fn value(&self) -> Float {
        self.value.clone()
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension.clone()
    }

    pub fn basis(&self) -> Option<&Basis> {
        self.basis.as_ref()
    }

    pub fn display(&self) -> &DisplayHints {
        &self.display
    }

    pub fn with_display(mut self, display: DisplayHints) -> Self {
        self.display = display;
        self
    }

    pub fn is_repeating_candidate(&self) -> bool {
        self.display.repeating_candidate
    }

    /// The zoned datetime this value names, if it is a temporal point.
    pub fn as_zoned(&self) -> Option<Zoned> {
        let point = self.dimension.as_temporal_point()?;
        zoned_from_epoch_seconds(&self.value, point.zone()?)
    }

    pub fn simplify(&mut self) {
        self.dimension = self.dimension.simplify();
    }

    pub fn convert(&self, new_dimension: &Dimension) -> Option<Self> {
        // Affine units are points on an axis, so they transform rather than
        // scale, and they only ever convert into their own kind.
        match (self.dimension.as_affine(), new_dimension.as_affine()) {
            (Some(Affine::Temperature(from)), Some(Affine::Temperature(to))) => {
                let kelvin = from.to_kelvin(&self.value);
                return Some(Self {
                    dimension: new_dimension.clone(),
                    value: to.from_kelvin(&kelvin),
                    basis: None,
                    display: self.display.clone(),
                });
            }
            // Epoch seconds are zone-independent, so this is a pure relabel.
            // The target names a zone; what kind of point it is comes from the
            // value, so a clock read elsewhere is still a clock.
            (Some(point), Some(target)) if point.zone().is_some() && target.zone().is_some() => {
                let zone = target.zone().expect("a temporal target names a zone");
                let display = if point.is_clock() {
                    self.display.clone()
                } else {
                    // Offsets are not whole days, so a bare date would become a lie.
                    self.display.clone().at_least(Granularity::Minute)
                };
                return Some(Self {
                    dimension: point_dimension(&point, zone),
                    value: self.value.clone(),
                    basis: None,
                    display,
                });
            }
            (Some(point), _) if point.zone().is_some() => return None,
            (_, Some(target)) if target.zone().is_some() => return None,
            _ => {}
        }

        // A calendar reading uses the real endpoints to convert between duration units.
        // E.x. 1 month is 30 days in a 30-day month, but 28 days in February.
        if let Some(basis) = &self.basis
            && new_dimension.is_duration()
        {
            let readings = basis.readings();
            let value = match new_dimension.as_calendar_unit() {
                Some(unit) => readings.project(unit),
                None => seconds_dimension()
                    .get_ratio(new_dimension)
                    .map(|ratio| readings.seconds.clone().div(&ratio)),
            };
            if let Some(value) = value {
                return Some(Self {
                    dimension: new_dimension.clone(),
                    value,
                    basis: Some(basis.clone()),
                    display: DisplayHints {
                        span_readings: false,
                        ..self.display.clone()
                    },
                });
            }
        }

        let ratio = self.dimension.get_ratio(new_dimension)?;
        Some(Self {
            dimension: new_dimension.clone(),
            value: self.value.clone().div(&ratio),
            // Conversion selects a reading; it does not reify one.
            basis: new_dimension
                .is_duration()
                .then(|| self.basis.clone())
                .flatten(),
            display: DisplayHints {
                span_readings: false,
                ..self.display.clone()
            },
        })
    }

    // A right-hand percentage is relative to the left value.
    pub fn try_add(&self, other: &Self) -> Option<Self> {
        self.additive(other, false)
    }

    pub fn try_sub(&self, other: &Self) -> Option<Self> {
        self.additive(other, true)
    }

    fn additive(&self, other: &Self, subtract: bool) -> Option<Self> {
        if let Some(result) = self.affine_additive(other, subtract) {
            return Some(result);
        }
        if self.dimension.is_temporal_point() || other.dimension.is_temporal_point() {
            return None;
        }

        if other.dimension.is_percent() && !self.dimension.is_percent() {
            let percentage = other.convert(&Dimension::default())?;
            let adjustment = self.value.clone().mul(&percentage.value);
            let value = if subtract {
                self.value.clone().sub(&adjustment)
            } else {
                self.value.clone().add(&adjustment)
            };
            return Some(Self {
                dimension: self.dimension.clone(),
                value,
                basis: None,
                display: self.display.merged(&other.display),
            });
        }
        if self.dimension.is_percent() && !other.dimension.is_percent() {
            return None;
        }

        let ratio = self.dimension.get_ratio(&other.dimension)?;
        let scaled = other.value.clone().mul(&ratio);
        let value = if subtract {
            self.value.clone().sub(&scaled)
        } else {
            self.value.clone().add(&scaled)
        };
        let basis = combined_basis(self, other, subtract);

        Some(Self {
            dimension: self.dimension.clone(),
            value,
            display: DisplayHints {
                span_readings: basis.is_some(),
                ..self.display.merged(&other.display)
            },
            basis,
        })
    }

    /// Temporal addition: point − point, point ± interval
    fn affine_additive(&self, other: &Self, subtract: bool) -> Option<Self> {
        let left = self.dimension.as_temporal_point();
        let right = other.dimension.as_temporal_point();

        match (left, right) {
            (Some(left), Some(right)) => {
                if !subtract || left.is_clock() != right.is_clock() {
                    return None;
                }
                // Two clock readings measure a stretch of the same day, which
                // no calendar axis disagrees about; hours are what people mean.
                if left.is_clock() {
                    return Some(Self {
                        dimension: hours_dimension(),
                        value: self.value.clone().sub(&other.value).div(&Float::from(3600)),
                        basis: None,
                        display: DisplayHints::default(),
                    });
                }
                let start = other.as_zoned()?;
                let end = self.as_zoned()?;
                Some(Self {
                    dimension: seconds_dimension(),
                    value: self.value.clone().sub(&other.value),
                    basis: Some(Basis::span(start, end)),
                    display: DisplayHints::readings(),
                })
            }
            (Some(point), None) => {
                if !other.dimension.is_duration() {
                    return None;
                }
                self.shift(other, &point, subtract)
            }
            (None, Some(point)) => {
                if subtract || !self.dimension.is_duration() {
                    return None;
                }
                other.shift(self, &point, false)
            }
            (None, None) => None,
        }
    }

    fn shift(&self, duration: &Value, point: &Affine, subtract: bool) -> Option<Self> {
        let default = if point.is_clock() {
            Granularity::Minute
        } else {
            Granularity::Date
        };
        let granularity = self
            .display
            .granularity
            .unwrap_or(default)
            .finer(duration_granularity(&duration.dimension));

        // Whole calendar units land on the same wall clock and clamp to the end
        // of a short month; anything else is exact seconds.
        if let Some(span) = calendar_span(duration) {
            let span = if subtract { span.negate() } else { span };
            if let Some(moment) = self.as_zoned().and_then(|at| at.checked_add(span).ok()) {
                return Some(Self::temporal_point(point, &moment, granularity));
            }
        }

        let seconds = duration.convert(&seconds_dimension())?;
        let value = if subtract {
            self.value.clone().sub(&seconds.value)
        } else {
            self.value.clone().add(&seconds.value)
        };
        Some(Self {
            dimension: self.dimension.clone(),
            value,
            basis: None,
            display: DisplayHints::instant(granularity),
        })
    }

    // Percentages contribute their scalar factor when multiplied or divided.
    pub fn try_mul(&self, other: &Self) -> Option<Self> {
        if self.dimension.is_percent() || other.dimension.is_percent() {
            let lhs = if self.dimension.is_percent() {
                self.convert(&Dimension::default())?
            } else {
                self.clone()
            };
            let rhs = if other.dimension.is_percent() {
                other.convert(&Dimension::default())?
            } else {
                other.clone()
            };
            return lhs.try_mul(&rhs);
        }

        let mut terms = self.dimension.0.clone();
        terms.extend(other.dimension.0.iter().cloned());

        let basis = scaled_basis(self, other);
        let mut result = Self {
            dimension: Dimension(terms).simplify().decay_affine(),
            value: self.value.clone().mul(&other.value),
            display: DisplayHints {
                span_readings: basis.is_some(),
                ..self.display.merged(&other.display)
            },
            basis,
        };

        if let Some(target) = self.dimension.collapse_target(&other.dimension)
            && let Some(collapsed) = result.convert(&target)
        {
            result = collapsed;
        }

        Some(result)
    }

    pub fn reciprocal(&self) -> Self {
        Self {
            dimension: Dimension(
                self.dimension
                    .0
                    .iter()
                    .map(|(quantity, power)| (quantity.clone(), { power.clone().neg() }))
                    .collect(),
            ),
            value: self.value.clone().recip(),
            basis: None,
            display: self.display.clone(),
        }
    }

    pub fn negate(&self) -> Self {
        Self {
            dimension: self.dimension.clone(),
            value: self.value.clone().neg(),
            basis: self.basis.as_ref().map(Basis::negated),
            display: self.display.clone(),
        }
    }

    pub fn try_div(&self, other: &Self) -> Option<Self> {
        if self.dimension.is_percent() || other.dimension.is_percent() {
            let lhs = if self.dimension.is_percent() {
                self.convert(&Dimension::default())?
            } else {
                self.clone()
            };
            let rhs = if other.dimension.is_percent() {
                other.convert(&Dimension::default())?
            } else {
                other.clone()
            };
            if rhs.value == Float::from(0) {
                return None;
            }
            return lhs.try_div(&rhs);
        }

        if other.value == Float::from(0) {
            return None;
        }
        let mut result = self.try_mul(&other.reciprocal())?;
        result.display.repeating_candidate = true;
        Some(result)
    }

    pub fn try_pow(&self, other: &Self) -> Option<Self> {
        if !other.dimension.is_unitless() {
            return None;
        }

        let power = &other.value;

        let new_dimension = self.dimension.pow(power).decay_affine();

        let value = if power.is_integer() {
            self.value.clone().pow(power)
        } else {
            self.value.clone().pow(power).round_inexact()
        };

        Some(Self {
            dimension: new_dimension,
            value,
            basis: None,
            display: DisplayHints {
                repeating_candidate: power.is_integer() && self.display.repeating_candidate,
                ..DisplayHints::default()
            },
        })
    }

    pub fn is_unitless(&self) -> bool {
        self.dimension.0.is_empty()
    }

    pub fn with_units(&self, dimension: Dimension) -> Self {
        Self {
            dimension,
            value: self.value.clone(),
            basis: self.basis.clone(),
            display: self.display.clone(),
        }
    }
}

/// Addition of two durations made of spans (with different answers for different granularities) should
/// produce a new basis representing the sum of each of those different answers, rather than collapsing.
/// E.X. 1 month + 1 month = 2 months, but 1 month + 1 month = 59 days in February, and 60 days in March.
fn combined_basis(left: &Value, right: &Value, subtract: bool) -> Option<Basis> {
    let (left, right) = (left.basis.as_ref()?, right.basis.as_ref()?);
    Some(if subtract {
        left.subtracted(right)
    } else {
        left.added(right)
    })
}

fn scaled_basis(left: &Value, right: &Value) -> Option<Basis> {
    match (&left.basis, &right.basis) {
        (Some(basis), None) if right.dimension.is_unitless() => Some(basis.scaled(&right.value)),
        (None, Some(basis)) if left.dimension.is_unitless() => Some(basis.scaled(&left.value)),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::representations::value::dimension::{DerivedUnit, Quantity, Unit};

    fn base(quantity: Quantity, power: &str) -> (Unit, Float) {
        (Unit::Base(quantity), Float::parse(power).unwrap())
    }

    #[test]
    fn test_conversion() {
        let area = Value::new(
            Float::parse("2.0").unwrap(),
            Some(Dimension(vec![base(
                Quantity::Length(Length::Kilometer),
                "2.0",
            )])),
        );

        let area = area.convert(&Dimension(vec![base(
            Quantity::Length(Length::Meter),
            "2.0",
        )]));

        assert_eq!(
            area.unwrap().value.to_string(),
            Float::parse("2000000").unwrap().to_string()
        );
    }

    #[test]
    fn value_try_add_returns_some_for_compatible_dimensions() {
        let value1 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![base(Quantity::Length(Length::Meter), "1")])),
        );

        let value2 = Value::new(
            Float::parse("52").unwrap(),
            Some(Dimension(vec![base(Quantity::Length(Length::Meter), "1")])),
        );

        let result = value1.try_add(&value2);

        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Float::parse("54").unwrap());
    }

    #[test]
    fn value_try_add_returns_none_for_incompatible_dimensions() {
        let value1 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![base(Quantity::Time(Time::Second), "1")])),
        );

        let value2 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![base(Quantity::Length(Length::Meter), "1")])),
        );

        assert!(value1.try_add(&value2).is_none());
    }

    #[test]
    fn value_try_mul_returns_some() {
        let value1 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![base(Quantity::Length(Length::Meter), "2")])),
        );

        let value2 = Value::new(
            Float::parse("3").unwrap(),
            Some(Dimension(vec![base(Quantity::Time(Time::Second), "3")])),
        );

        let result = value1.try_mul(&value2);

        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Float::parse("6").unwrap());
    }

    #[test]
    fn value_try_div_returns_some() {
        let value1 = Value::new(
            Float::parse("10").unwrap(),
            Some(Dimension(vec![base(Quantity::Length(Length::Meter), "2")])),
        );

        let value2 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![base(Quantity::Length(Length::Meter), "2")])),
        );

        let result = value1.try_div(&value2);

        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Float::parse("5").unwrap());
    }

    #[test]
    fn derived_unit_converts_to_base_expansion() {
        let speed = Value::new(
            Float::from(60),
            Some(Dimension(vec![(
                Unit::Derived(DerivedUnit::MilePerHour),
                Float::from(1),
            )])),
        );

        let as_m_per_s = Dimension(vec![
            base(Quantity::Length(Length::Meter), "1"),
            base(Quantity::Time(Time::Second), "-1"),
        ]);

        let converted = speed
            .convert(&as_m_per_s)
            .expect("mph -> m/s should convert");
        let expected = &Float::from(60) * &(&Length::Mile.ratio() / &Time::Hour.ratio());
        assert_eq!(converted.value.to_string(), expected.to_string());
    }

    #[test]
    fn torque_and_energy_share_dimensionality_but_display_differently() {
        let torque = Dimension(vec![
            (Unit::Derived(DerivedUnit::Newton), Float::from(1)),
            base(Quantity::Length(Length::Meter), "1"),
        ]);
        let energy = Dimension(vec![(Unit::Derived(DerivedUnit::Joule), Float::from(1))]);

        assert_eq!(torque.dimensionality(), energy.dimensionality());
        assert_eq!(torque.to_string(), "N*m");
        assert_eq!(energy.to_string(), "J");
    }

    #[test]
    fn reverse_lookup_finds_named_equivalent() {
        let force = Dimension(vec![
            base(Quantity::Mass(Mass::Kilogram, None), "1"),
            base(Quantity::Length(Length::Meter), "1"),
            base(Quantity::Time(Time::Second), "-2"),
        ]);

        assert!(force.named_equivalents().contains(&DerivedUnit::Newton));
        assert_eq!(force.collapse_to_named().unwrap().to_string(), "N");
    }
}
