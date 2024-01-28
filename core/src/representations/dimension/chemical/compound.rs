use crate::representations::Element;
use std::collections::BTreeMap;
use std::fmt::Display;
use crate::representations::dimension::chemical::parse::parse_compound;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Compound(pub BTreeMap<Element, u16>);

impl Compound {
    pub fn parse(formula: &str) -> Option<Self> {
        parse_compound(formula)
    }
    pub fn particulate_mass(&self) -> f32 {
        self.0
            .iter()
            .map(|(element, count)| element.atomic_mass() * *count as f32)
            .sum()
    }
}

impl Display for Compound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut formula = String::new();
        for (element, count) in &self.0 {
            formula.push_str(&element.symbol());
            if *count > 1 {
                formula.push_str(&count.to_string());
            }
        }
        write!(f, "{}", formula)
    }
}
