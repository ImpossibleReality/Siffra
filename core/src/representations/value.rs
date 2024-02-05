use std::ops::{Add, Div, Mul, Neg, Sub};

pub use crate::representations::*;

#[derive(Debug, Clone)]
pub struct Value {
    pub dimension: Dimension,
    pub value: Float,
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self {
            dimension: Default::default(),
            value: Float::from(value),
        }
    }
}

impl Value {
    pub fn new(value: Float, dimension: Option<Dimension>) -> Self {
        Self {
            dimension: dimension.unwrap_or(Default::default()),
            value,
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

    pub fn simplify(&mut self) {
        self.dimension.simplify();
    }

    pub fn convert(&self, new_dimension: &Dimension) -> Option<Self> {
        let ratio = self.dimension.get_ratio(new_dimension)?;
        Some(Self {
            dimension: new_dimension.clone(),
            value: self.value.clone().div(&ratio),
        })
    }

    pub fn try_add(&self, other: &Self) -> Option<Self> {
        let ratio = self.dimension.get_ratio(&other.dimension)?;

        Some(Self {
            dimension: self.dimension.clone(),
            value: self.value.clone().add(&other.value.clone().mul(&ratio)),
        })
    }

    pub fn try_sub(&self, other: &Self) -> Option<Self> {
        let ratio = self.dimension.get_ratio(&other.dimension)?;

        Some(Self {
            dimension: self.dimension.clone(),
            value: self.value.clone().sub(&other.value.clone().mul(&ratio)),
        })
    }

    pub fn try_mul(&self, other: &Self) -> Option<Self> {
        let qmap = self.dimension.get_quantity_map();
        let mut new_dimension = other.dimension.clone();
        new_dimension.apply_quantity_map(&qmap);

        let other = other.convert(&new_dimension).expect("Conversion failed");

        let mut dim = self.dimension.0.clone();
        for (quantity, power) in new_dimension.0.iter() {
            dim.push((quantity.clone(), power.clone()));
        }

        Some(Self {
            dimension: Dimension(dim).simplify(),
            value: self.value.clone().mul(&other.value),
        })
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
        }
    }

    pub fn try_div(&self, other: &Self) -> Option<Self> {
        self.try_mul(&other.reciprocal())
    }

    pub fn try_pow(&self, other: &Self) -> Option<Self> {
        if !other.dimension.is_unitless() {
            return None;
        }

        let power = &other.value;

        let new_dimension = self.dimension.pow(power);

        let value = self.value.clone().pow(power);

        Some(Self {
            dimension: new_dimension,
            value,
        })
    }

    pub fn is_unitless(&self) -> bool {
        self.dimension.0.is_empty()
    }

    pub fn with_units(&self, dimension: Dimension) -> Self {
        Self {
            dimension,
            value: self.value.clone(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::representations::value::dimension::Quantity;

    #[test]
    fn test_conversion() {
        let area = Value::new(
            Float::parse("2.0").unwrap(),
            Some(Dimension(vec![(
                Quantity::Length(Length::Kilometer),
                Float::parse("2.0").unwrap(),
            )])),
        );

        let area = area.convert(&Dimension(vec![(
            Quantity::Length(Length::Meter),
            Float::parse("2.0").unwrap(),
        )]));

        assert_eq!(
            area.unwrap().value.to_string(),
            Float::parse("2000000").unwrap().to_string()
        );
    }

    #[test]
    fn dimension_sanity_check_returns_true_for_sane_dimension() {
        let dimension = Dimension(vec![
            (Quantity::Length(dimension::Length::Meter), Float::from(2)),
            (Quantity::Time(dimension::Time::Second), Float::from(1)),
        ]);

        assert!(dimension.sanity_check());
    }

    #[test]
    fn dimension_sanity_check_returns_false_for_insane_dimension() {
        let dimension = Dimension(vec![
            (Quantity::Length(dimension::Length::Meter), Float::from(2)),
            (
                Quantity::Length(dimension::Length::Kilometer),
                Float::from(1),
            ),
        ]);

        assert!(!dimension.sanity_check());
    }

    #[test]
    fn value_try_add_returns_some_for_compatible_dimensions() {
        let value1 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![(
                Quantity::Length(dimension::Length::Meter),
                Float::parse("1").unwrap(),
            )])),
        );

        let value2 = Value::new(
            Float::parse("52").unwrap(),
            Some(Dimension(vec![(
                Quantity::Length(dimension::Length::Meter),
                Float::parse("1").unwrap(),
            )])),
        );

        let result = value1.try_add(&value2);

        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Float::parse("54").unwrap());
    }

    #[test]
    fn value_try_add_returns_none_for_incompatible_dimensions() {
        let value1 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![(
                Quantity::Time(dimension::Time::Second),
                Float::parse("1").unwrap(),
            )])),
        );

        let value2 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![(
                Quantity::Length(dimension::Length::Meter),
                Float::parse("1").unwrap(),
            )])),
        );

        let result = value1.try_add(&value2);

        assert!(result.is_none());
    }

    #[test]
    fn value_try_mul_returns_some() {
        let value1 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![(
                Quantity::Length(dimension::Length::Meter),
                Float::parse("2").unwrap(),
            )])),
        );

        let value2 = Value::new(
            Float::parse("3").unwrap(),
            Some(Dimension(vec![(
                Quantity::Time(dimension::Time::Second),
                Float::parse("3").unwrap(),
            )])),
        );

        let result = value1.try_mul(&value2);

        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Float::parse("6").unwrap());
    }

    #[test]
    fn value_try_div_returns_some() {
        let value1 = Value::new(
            Float::parse("10").unwrap(),
            Some(Dimension(vec![(
                Quantity::Length(dimension::Length::Meter),
                Float::parse("2").unwrap(),
            )])),
        );

        let value2 = Value::new(
            Float::parse("2").unwrap(),
            Some(Dimension(vec![(
                Quantity::Length(dimension::Length::Meter),
                Float::parse("2").unwrap(),
            )])),
        );

        let result = value1.try_div(&value2);

        assert!(result.is_some());
        assert_eq!(result.unwrap().value, Float::parse("5").unwrap());
    }
}
