use crate::representations::dimension::chemical::parse::parse_compound;
use crate::representations::{Element, Float};
use std::collections::BTreeMap;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Compound(pub BTreeMap<Element, u16>);

impl Compound {
    pub fn parse(formula: &str) -> Option<Self> {
        parse_compound(formula)
    }
    pub fn particulate_mass(&self) -> Float {
        self.0
            .iter()
            .fold(Float::from(0), |total, (element, count)| {
                &total + &(&element.atomic_mass() * &Float::from(*count))
            })
    }
}

impl Display for Compound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut formula = String::new();
        for (element, count) in &self.0 {
            formula.push_str(element.symbol());
            if *count > 1 {
                formula.push_str(&count.to_string());
            }
        }
        write!(f, "{}", formula)
    }
}
