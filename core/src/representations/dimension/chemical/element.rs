use crate::representations::Float;
use mendeleev::Element as MendeleevElement;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Element {
    element: MendeleevElement,
}

impl Element {
    pub fn from_symbol(sym: &str) -> Option<Element> {
        let mut chars = sym.chars();
        if !matches!(chars.next(), Some(c) if c.is_ascii_uppercase()) {
            return None;
        }
        if !chars.all(|c| c.is_ascii_lowercase()) {
            return None;
        }
        MendeleevElement::list()
            .iter()
            .copied()
            .find(|element| element.symbol() == sym)
            .map(Element::from_mendeleev_element)
    }

    pub fn from_atomic_number(z: usize) -> Option<Element> {
        MendeleevElement::list()
            .get(z.checked_sub(1)?)
            .copied()
            .map(Element::from_mendeleev_element)
    }

    fn from_mendeleev_element(element: MendeleevElement) -> Element {
        Element { element }
    }

    pub fn symbol(&self) -> &'static str {
        self.element.symbol()
    }

    pub fn atomic_mass(&self) -> Float {
        Float::parse(&f64::from(self.element.atomic_weight()).to_string())
            .expect("valid atomic weight")
    }
}

#[cfg(test)]
mod tests {
    use super::Element;

    #[test]
    fn uses_mendeleev_standard_atomic_weights() {
        assert_eq!(
            Element::from_symbol("H").unwrap().atomic_mass().to_string(),
            "1.008"
        );
        assert_eq!(
            Element::from_atomic_number(8)
                .unwrap()
                .atomic_mass()
                .to_string(),
            "15.999"
        );
    }
}
