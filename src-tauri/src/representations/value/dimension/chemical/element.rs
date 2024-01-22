use periodic_table_on_an_enum::Element as PElement;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Element {
    element: PElement,
}

impl Element {
    pub fn from_symbol(sym: &str) -> Option<Element> {
        PElement::from_symbol(sym).map(Element::from_pt_element)
    }

    pub fn from_atomic_number(z: usize) -> Option<Element> {
        PElement::from_atomic_number(z).map(Element::from_pt_element)
    }

    fn from_pt_element(element: PElement) -> Element {
        Element { element }
    }

    pub fn symbol(&self) -> &'static str {
        self.element.get_symbol()
    }

    pub fn atomic_mass(&self) -> f32 {
        self.element.get_atomic_mass()
    }
}
