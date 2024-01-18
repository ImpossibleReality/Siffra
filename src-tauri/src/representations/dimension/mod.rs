use crate::representations::dimension::chemical::Compound;
use num::BigRational;
use num::FromPrimitive;
use std::collections::{BTreeMap, HashMap};
use std::ops::{Add, Div, Mul, Sub};
use std::str::FromStr;

mod amount;
mod chemical;
mod length;
mod macros;
mod mass;
mod time;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuantityKind {
    Length,
    Time,
    Mass,
    Amount,
    Compound(Compound),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Quantity {
    Length(length::Length),
    Time(time::Time),
    Mass(mass::Mass, Option<Compound>),
    Amount(amount::Amount, Option<Compound>),
}

impl FromStr for Quantity {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if length::Length::from_str(s).is_ok() {
            return Ok(Quantity::Length(length::Length::from_str(s).unwrap()));
        }

        if time::Time::from_str(s).is_ok() {
            return Ok(Quantity::Time(time::Time::from_str(s).unwrap()));
        }

        if mass::Mass::from_str(s).is_ok() {
            return Ok(Quantity::Mass(mass::Mass::from_str(s).unwrap(), None));
        }

        if amount::Amount::from_str(s).is_ok() {
            return Ok(Quantity::Amount(amount::Amount::from_str(s).unwrap(), None));
        }

        Err(())
    }
}

impl Quantity {
    pub fn quantity_kind(&self) -> QuantityKind {
        match self {
            Quantity::Length(_) => QuantityKind::Length,
            Quantity::Time(_) => QuantityKind::Time,
            Quantity::Mass(_, compound) => match compound {
                Some(compound) => QuantityKind::Compound(compound.clone()),
                None => QuantityKind::Mass,
            },
            Quantity::Amount(_, compound) => match compound {
                Some(compound) => QuantityKind::Compound(compound.clone()),
                None => QuantityKind::Amount,
            },
        }
    }

    pub fn get_ratio(&self) -> BigRational {
        match self {
            Quantity::Length(length) => length.ratio(),
            Quantity::Time(time) => time.ratio(),
            Quantity::Mass(mass, _) => mass.ratio(),
            Quantity::Amount(amount, _) => amount.ratio(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Dimension(Vec<(Quantity, i32)>);

impl Dimension {
    pub fn new(quantities: Vec<(Quantity, i32)>) -> Self {
        Self(quantities).simplify()
    }
    pub fn simplify(&self) -> Self {
        // add together all quantities with the same unit
        let mut new_dimension = Vec::new();

        for (quantity, power) in self.0.iter() {
            let mut found = false;
            for (new_quantity, new_power) in new_dimension.iter_mut() {
                if new_quantity == quantity {
                    *new_power += power;
                    found = true;
                    break;
                }
            }
            if !found {
                new_dimension.push((quantity.clone(), *power));
            }
        }

        Dimension(new_dimension)
    }

    fn sanity_check(&self) -> bool {
        // check that all quantities of same kind have the same unit
        let mut quantities: BTreeMap<QuantityKind, Quantity> = BTreeMap::new();

        for (quantity, _) in self.0.iter() {
            let quantity_kind = quantity.quantity_kind();
            if let Some(existing_quantity) = quantities.get(&quantity_kind) {
                if existing_quantity != quantity {
                    return false;
                }
            } else {
                quantities.insert(quantity_kind, quantity.clone());
            }
        }

        true
    }

    pub fn get_quantity_map(&self) -> BTreeMap<QuantityKind, Quantity> {
        let mut quantities: BTreeMap<QuantityKind, Quantity> = BTreeMap::new();

        for (quantity, _) in self.0.iter() {
            let quantity_kind = quantity.quantity_kind();
            if let Some(existing_quantity) = quantities.get(&quantity_kind) {
                if existing_quantity != quantity {
                    panic!("Dimension is not sane");
                }
            } else {
                quantities.insert(quantity_kind, quantity.clone());
            }
        }

        quantities
    }

    pub fn apply_quantity_map(&mut self, quantity_map: &BTreeMap<QuantityKind, Quantity>) {
        for (quantity, power) in self.0.iter_mut() {
            let quantity_kind = quantity.quantity_kind();
            if let Some(new_quantity) = quantity_map.get(&quantity_kind) {
                *quantity = new_quantity.clone();
            }
        }
    }

    /// Returns the ratio of this dimension to another dimension.
    pub fn get_ratio(&self, other: &Self) -> Option<BigRational> {
        if !self.sanity_check() || !other.sanity_check() {
            return None;
        }

        if self == other {
            return Some(BigRational::from_integer(num::BigInt::from(1)));
        }

        let mut ratio = BigRational::from_integer(num::BigInt::from(1));

        for (quantity, power) in self.0.iter() {
            let mut found = false;
            for (other_quantity, other_power) in other.0.iter() {
                if quantity.quantity_kind() == other_quantity.quantity_kind() {
                    if *power != *other_power {
                        return None;
                    }
                    let quantity_ratio = quantity.get_ratio().pow(*power)
                        / other_quantity.get_ratio().pow(*other_power);
                    ratio /= quantity_ratio;
                    found = true;
                }
            }
            if !found {
                return None;
            }
        }

        Some(ratio)
    }
}

#[derive(Debug, Clone)]
pub struct Value {
    pub dimension: Dimension,
    pub value: BigRational,
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self {
            dimension: Default::default(),
            value: BigRational::from_f64(value).unwrap(),
        }
    }
}

impl Value {
    pub fn new(value: BigRational, dimension: Option<Dimension>) -> Self {
        Self {
            dimension: dimension.unwrap_or(Default::default()),
            value,
        }
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
            value: self.value.clone().add(other.value.clone().mul(&ratio)),
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

        let other = other.convert(&new_dimension)?;

        let mut dim = self.dimension.0.clone();
        for (quantity, power) in new_dimension.0.iter() {
            dim.push((quantity.clone(), *power));
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
                    .map(|(quantity, power)| (quantity.clone(), -power))
                    .collect(),
            ),
            value: self.value.clone().recip(),
        }
    }

    pub fn try_div(&self, other: &Self) -> Option<Self> {
        self.try_mul(&other.reciprocal())
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

    #[test]
    fn test_conversion() {
        let area = Value::new(
            BigRational::from_float(2.0).unwrap(),
            Some(Dimension(vec![(
                Quantity::Length(length::Length::Kilometer),
                2,
            )])),
        );

        let area = area.convert(&Dimension(vec![(
            Quantity::Length(length::Length::Meter),
            2,
        )]));

        assert_eq!(
            area.unwrap().value,
            BigRational::from_float(2000000.0).unwrap()
        );
    }

    #[test]
    fn dimension_sanity_check_returns_true_for_sane_dimension() {
        let dimension = Dimension(vec![
            (Quantity::Length(length::Length::Meter), 2),
            (Quantity::Time(time::Time::Second), 1),
        ]);

        assert!(dimension.sanity_check());
    }

    #[test]
    fn dimension_sanity_check_returns_false_for_insane_dimension() {
        let dimension = Dimension(vec![
            (Quantity::Length(length::Length::Meter), 2),
            (Quantity::Length(length::Length::Kilometer), 1),
        ]);

        assert!(!dimension.sanity_check());
    }

    #[test]
    fn value_try_add_returns_some_for_compatible_dimensions() {
        let value1 = Value::new(
            BigRational::from_integer(num::BigInt::from(2)),
            Some(Dimension(vec![(
                Quantity::Length(length::Length::Meter),
                1,
            )])),
        );
        let value2 = Value::new(
            BigRational::from_integer(num::BigInt::from(3)),
            Some(Dimension(vec![(
                Quantity::Length(length::Length::Meter),
                1,
            )])),
        );

        let result = value1.try_add(&value2);

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().value,
            BigRational::from_integer(num::BigInt::from(5))
        );
    }

    #[test]
    fn value_try_add_returns_none_for_incompatible_dimensions() {
        let value1 = Value::new(
            BigRational::from_integer(num::BigInt::from(2)),
            Some(Dimension(vec![(
                Quantity::Length(length::Length::Meter),
                1,
            )])),
        );
        let value2 = Value::new(
            BigRational::from_integer(num::BigInt::from(3)),
            Some(Dimension(vec![(Quantity::Time(time::Time::Second), 1)])),
        );

        let result = value1.try_add(&value2);

        assert!(result.is_none());
    }

    #[test]
    fn value_try_mul_returns_some() {
        let value1 = Value::new(
            BigRational::from_integer(num::BigInt::from(2)),
            Some(Dimension(vec![(
                Quantity::Length(length::Length::Meter),
                1,
            )])),
        );
        let value2 = Value::new(
            BigRational::from_integer(num::BigInt::from(3)),
            Some(Dimension(vec![(Quantity::Time(time::Time::Second), 1)])),
        );

        let result = value1.try_mul(&value2);

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().value,
            BigRational::from_integer(num::BigInt::from(6))
        );
    }

    #[test]
    fn value_try_div_returns_some() {
        let value1 = Value::new(
            BigRational::from_integer(num::BigInt::from(2)),
            Some(Dimension(vec![(
                Quantity::Length(length::Length::Meter),
                1,
            )])),
        );
        let value2 = Value::new(
            BigRational::from_integer(num::BigInt::from(1)),
            Some(Dimension(vec![(
                Quantity::Length(length::Length::Meter),
                1,
            )])),
        );

        let result = value1.try_div(&value2);

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().value,
            BigRational::from_integer(num::BigInt::from(2))
        );
    }
}
