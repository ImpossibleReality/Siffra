use crate::representations::Float;
use periodic_table_on_an_enum::Element as PElement;

const ATOMIC_MASSES: [&str; 118] = [
    "1.00794",
    "4.002602",
    "6.941",
    "9.012182",
    "10.811",
    "12.0107",
    "14.0067",
    "15.9994",
    "18.9984032",
    "20.1797",
    "22.98976928",
    "24.3050",
    "26.9815386",
    "28.0855",
    "30.973762",
    "32.065",
    "35.453",
    "39.948",
    "39.0983",
    "40.078",
    "44.955912",
    "47.867",
    "50.9415",
    "51.9961",
    "54.938045",
    "55.845",
    "58.933195",
    "58.6934",
    "63.546",
    "65.38",
    "69.723",
    "72.64",
    "74.92160",
    "78.96",
    "79.904",
    "83.798",
    "85.4678",
    "87.62",
    "88.90585",
    "91.224",
    "92.90638",
    "95.96",
    "98",
    "101.07",
    "102.90550",
    "106.42",
    "107.8682",
    "112.411",
    "114.818",
    "118.710",
    "121.760",
    "127.60",
    "126.90447",
    "131.293",
    "132.9054519",
    "137.327",
    "138.90547",
    "140.116",
    "140.90765",
    "144.242",
    "145",
    "150.36",
    "151.964",
    "157.25",
    "158.92535",
    "162.500",
    "164.93032",
    "167.259",
    "168.93421",
    "173.054",
    "174.9668",
    "178.49",
    "180.94788",
    "183.84",
    "186.207",
    "190.23",
    "192.217",
    "195.084",
    "196.966569",
    "200.59",
    "204.3833",
    "207.2",
    "208.98040",
    "209",
    "210",
    "222",
    "223",
    "226",
    "227",
    "232.03806",
    "231.03588",
    "238.02891",
    "237",
    "244",
    "243",
    "247",
    "247",
    "251",
    "252",
    "257",
    "258",
    "259",
    "262",
    "267",
    "268",
    "271",
    "274",
    "277",
    "278",
    "281",
    "282",
    "285",
    "286",
    "289",
    "290",
    "293",
    "294",
    "294",
];

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct Element {
    element: PElement,
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

    pub fn atomic_mass(&self) -> Float {
        let index = self.element.get_atomic_number() - 1;
        Float::parse(ATOMIC_MASSES[index]).expect("valid atomic mass")
    }
}
