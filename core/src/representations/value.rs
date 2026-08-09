use std::ops::{Add, Div, Mul, Neg, Sub};

pub use crate::representations::*;

#[derive(Debug, Clone)]
pub struct Value {
    pub dimension: Dimension,
    pub value: Float,
    repeating_candidate: bool,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.dimension == other.dimension && self.value == other.value
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self {
            dimension: Default::default(),
            value: Float::from(value),
            repeating_candidate: false,
        }
    }
}

impl Value {
    pub fn new(value: Float, dimension: Option<Dimension>) -> Self {
        Self {
            dimension: dimension.unwrap_or_default(),
            value,
            repeating_candidate: false,
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

    pub fn is_repeating_candidate(&self) -> bool {
        self.repeating_candidate
    }

    pub fn simplify(&mut self) {
        self.dimension.simplify();
    }

    pub fn convert(&self, new_dimension: &Dimension) -> Option<Self> {
        // Lone absolute temperatures use an affine transform, not a ratio.
        if let (Some(from), Some(to)) = (
            self.dimension.as_absolute_temperature(),
            new_dimension.as_absolute_temperature(),
        ) {
            let kelvin = from.to_kelvin(&self.value);
            return Some(Self {
                dimension: new_dimension.clone(),
                value: to.from_kelvin(&kelvin),
                repeating_candidate: self.repeating_candidate,
            });
        }

        let ratio = self.dimension.get_ratio(new_dimension)?;
        Some(Self {
            dimension: new_dimension.clone(),
            value: self.value.clone().div(&ratio),
            repeating_candidate: self.repeating_candidate,
        })
    }

    // A right-hand percentage is relative to the left value.
    pub fn try_add(&self, other: &Self) -> Option<Self> {
        if other.dimension.is_percent() && !self.dimension.is_percent() {
            let percentage = other.convert(&Dimension::default())?;
            let adjustment = self.value.clone().mul(&percentage.value);
            return Some(Self {
                dimension: self.dimension.clone(),
                value: self.value.clone().add(&adjustment),
                repeating_candidate: self.repeating_candidate || other.repeating_candidate,
            });
        }
        if self.dimension.is_percent() && !other.dimension.is_percent() {
            return None;
        }

        let ratio = self.dimension.get_ratio(&other.dimension)?;

        Some(Self {
            dimension: self.dimension.clone(),
            value: self.value.clone().add(&other.value.clone().mul(&ratio)),
            repeating_candidate: self.repeating_candidate || other.repeating_candidate,
        })
    }

    pub fn try_sub(&self, other: &Self) -> Option<Self> {
        if other.dimension.is_percent() && !self.dimension.is_percent() {
            let percentage = other.convert(&Dimension::default())?;
            let adjustment = self.value.clone().mul(&percentage.value);
            return Some(Self {
                dimension: self.dimension.clone(),
                value: self.value.clone().sub(&adjustment),
                repeating_candidate: self.repeating_candidate || other.repeating_candidate,
            });
        }
        if self.dimension.is_percent() && !other.dimension.is_percent() {
            return None;
        }

        let ratio = self.dimension.get_ratio(&other.dimension)?;

        Some(Self {
            dimension: self.dimension.clone(),
            value: self.value.clone().sub(&other.value.clone().mul(&ratio)),
            repeating_candidate: self.repeating_candidate || other.repeating_candidate,
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

        let mut result = Self {
            dimension: Dimension(terms).simplify().decay_absolute(),
            value: self.value.clone().mul(&other.value),
            repeating_candidate: self.repeating_candidate || other.repeating_candidate,
        };

        if let Some(target) = self.dimension.collapse_target(&other.dimension)
            && let Some(collapsed) = result.convert(&target) {
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
            repeating_candidate: self.repeating_candidate,
        }
    }

    pub fn negate(&self) -> Self {
        Self {
            dimension: self.dimension.clone(),
            value: self.value.clone().neg(),
            repeating_candidate: self.repeating_candidate,
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
        result.repeating_candidate = true;
        Some(result)
    }

    pub fn try_pow(&self, other: &Self) -> Option<Self> {
        if !other.dimension.is_unitless() {
            return None;
        }

        let power = &other.value;

        let new_dimension = self.dimension.pow(power).decay_absolute();

        let value = if power.is_integer() {
            self.value.clone().pow(power)
        } else {
            self.value.clone().pow(power).round_inexact()
        };

        Some(Self {
            dimension: new_dimension,
            value,
            repeating_candidate: power.is_integer() && self.repeating_candidate,
        })
    }

    pub fn is_unitless(&self) -> bool {
        self.dimension.0.is_empty()
    }

    pub fn with_units(&self, dimension: Dimension) -> Self {
        Self {
            dimension,
            value: self.value.clone(),
            repeating_candidate: self.repeating_candidate,
        }
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
