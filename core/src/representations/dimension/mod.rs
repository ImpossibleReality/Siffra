mod amount;
mod angle;
mod chemical;
mod length;
mod macros;
mod mass;
mod time;

use crate::representations::Float;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::Neg;
use std::str::FromStr;

pub use {
    amount::Amount,
    angle::Angle,
    chemical::{Compound, Element},
    length::Length,
    mass::Mass,
    time::Time,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuantityKind {
    Length,
    Time,
    Mass,
    Amount,
    Angle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Quantity {
    Length(length::Length),
    Time(time::Time),
    Mass(mass::Mass, Option<Compound>),
    Amount(amount::Amount, Option<Compound>),
    Angle(angle::Angle),
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

        if angle::Angle::from_str(s).is_ok() {
            return Ok(Quantity::Angle(angle::Angle::from_str(s).unwrap()));
        }

        Err(())
    }
}

impl Quantity {
    pub fn with_chemical(self, compound: Compound) -> Self {
        match self {
            Quantity::Mass(mass, _) => Quantity::Mass(mass, Some(compound)),
            Quantity::Amount(amount, _) => Quantity::Amount(amount, Some(compound)),
            _ => self,
        }
    }

    pub fn quantity_kind(&self) -> QuantityKind {
        match self {
            Quantity::Length(_) => QuantityKind::Length,
            Quantity::Time(_) => QuantityKind::Time,
            Quantity::Mass(_, _) => QuantityKind::Mass,
            Quantity::Amount(_, _) => QuantityKind::Amount,
            Quantity::Angle(_) => QuantityKind::Angle,
        }
    }

    pub fn shorthand(&self) -> String {
        match self {
            Quantity::Length(length) => length.shorthand().to_string(),
            Quantity::Time(time) => time.shorthand().to_string(),
            Quantity::Mass(mass, compound) => {
                if let Some(compound) = compound {
                    mass.shorthand().to_string() + " " + &*compound.to_string()
                } else {
                    mass.shorthand().to_string()
                }
            }
            Quantity::Amount(amount, compound) => {
                if let Some(compound) = compound {
                    amount.shorthand().to_string() + " " + &*compound.to_string()
                } else {
                    amount.shorthand().to_string()
                }
            }
            Quantity::Angle(angle) => angle.shorthand().to_string(),
        }
    }

    pub fn get_ratio(&self) -> Float {
        match self {
            Quantity::Length(length) => length.ratio(),
            Quantity::Time(time) => time.ratio(),
            Quantity::Mass(mass, _) => mass.ratio(),
            Quantity::Amount(amount, _) => amount.ratio(),
            Quantity::Angle(angle) => angle.ratio(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Dimension(pub Vec<(Quantity, Float)>);

impl Display for Dimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut numerator = String::new();
        let mut denominator = String::new();

        for (quantity, power) in self.0.iter() {
            let mut quantity_shorthand = quantity.shorthand().to_string();

            if *power > Float::from(0) {
                if !numerator.is_empty() {
                    quantity_shorthand = format!("*{}", quantity_shorthand);
                }
                numerator.push_str(&*quantity_shorthand);
                if *power > Float::from(1) {
                    numerator.push_str(&format!("^{}", power));
                }
            } else if *power < Float::from(0) {
                if !denominator.is_empty() {
                    quantity_shorthand = format!("*{}", quantity_shorthand);
                }
                denominator.push_str(&*quantity_shorthand);
                if *power != Float::from(-1) {
                    denominator.push_str(&format!("^{}", power.clone().neg()));
                }
            }
        }

        if !numerator.is_empty() {
            write!(f, "{}", numerator)?;
        }

        if !denominator.is_empty() {
            write!(f, "/{}", denominator)?;
        }

        Ok(())
    }
}

impl Dimension {
    pub fn new(quantities: Vec<(Quantity, Float)>) -> Self {
        Self(quantities).simplify()
    }
    pub fn simplify(&self) -> Self {
        // add together all quantities with the same unit
        let mut new_dimension: Vec<(Quantity, Float)> = Vec::new();

        for (quantity, power) in self.0.iter() {
            let mut found = false;
            for (new_quantity, new_power) in new_dimension.iter_mut() {
                if new_quantity == quantity {
                    *new_power = power + new_power;
                    found = true;
                    break;
                }
            }
            if !found {
                new_dimension.push((quantity.clone(), power.clone()));
            }
        }

        // Remove any quantities with a power of 0
        new_dimension.retain(|(_, power)| *power != Float::from(0));

        Dimension(new_dimension)
    }

    pub fn sanity_check(&self) -> bool {
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
        for (quantity, _power) in self.0.iter_mut() {
            let quantity_kind = quantity.quantity_kind();
            if let Some(new_quantity) = quantity_map.get(&quantity_kind) {
                *quantity = new_quantity.clone();
            }
        }
    }

    pub fn pow(&self, power: &Float) -> Self {
        let mut new_dimension = Vec::new();

        for (quantity, old_power) in self.0.iter() {
            new_dimension.push((quantity.clone(), old_power * power));
        }

        Dimension(new_dimension).simplify()
    }

    /// Returns the ratio of this value to another value.
    pub fn get_ratio(&self, other: &Self) -> Option<Float> {
        if !self.sanity_check() || !other.sanity_check() {
            return None;
        }

        if self == other {
            return Some(Float::from(1));
        }

        let mut ratio = Float::from(1);

        for (quantity, power) in self.0.iter() {
            let mut found = false;
            for (other_quantity, other_power) in other.0.iter() {
                if quantity.quantity_kind() == other_quantity.quantity_kind() {
                    dbg!(quantity, other_quantity);

                    if *power != *other_power {
                        return None;
                    }

                    let mut quantity_ratio = &other_quantity.get_ratio() / &quantity.get_ratio();

                    quantity_ratio = quantity_ratio.pow(power);

                    ratio = &ratio * &quantity_ratio;
                    found = true;
                }
            }
            if !found {
                // Try to convert chemical to mass or amount
                if let Quantity::Mass(mass, Some(chemical)) = quantity {
                    for (other_quantity, other_power) in other.0.iter() {
                        if let Quantity::Amount(amount, other_chemical) = other_quantity {
                            if let Some(other_chemical) = other_chemical {
                                if other_chemical != chemical {
                                    continue;
                                }
                            }

                            if *power != Float::from(1.) && *other_power != Float::from(1.) {
                                continue;
                            }

                            // Convert amount to moles and mass to grams
                            let amount_ratio = &Amount::Mole.ratio() / &amount.ratio();
                            let mass_ratio = &Mass::Gram.ratio() / &mass.ratio();

                            let mut quantity_ratio = &mass_ratio / &amount_ratio;

                            // Divide by particulate mass
                            quantity_ratio = &quantity_ratio
                                * &Float::parse(&*chemical.particulate_mass().to_string()).unwrap();

                            ratio = &ratio * &quantity_ratio;
                            found = true;
                        }
                    }
                } else if let Quantity::Amount(amount, Some(chemical)) = quantity {
                    for (other_quantity, other_power) in other.0.iter() {
                        if let Quantity::Mass(mass, other_chemical) = other_quantity {
                            if let Some(other_chemical) = other_chemical {
                                if other_chemical != chemical {
                                    continue;
                                }
                            }

                            if *power != Float::from(1.) && *other_power != Float::from(1.) {
                                continue;
                            }

                            // Convert amount to moles and mass to grams
                            let amount_ratio = &Amount::Mole.ratio() / &amount.ratio();
                            let mass_ratio = &Mass::Gram.ratio() / &mass.ratio();

                            let mut quantity_ratio = &amount_ratio / &mass_ratio;

                            // Multiply by particulate mass
                            quantity_ratio = &quantity_ratio
                                / &Float::parse(&*chemical.particulate_mass().to_string()).unwrap();

                            ratio = &ratio * &quantity_ratio;
                            found = true;
                        }
                    }
                }
                if !found {
                    return None;
                }
            }
        }

        Some(ratio)
    }

    pub fn is_unitless(&self) -> bool {
        self.0.is_empty()
    }
}
