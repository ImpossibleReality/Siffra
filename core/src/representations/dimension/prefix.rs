use crate::representations::Float;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    Quetta,
    Ronna,
    Yotta,
    Zetta,
    Exa,
    Peta,
    Tera,
    Giga,
    Mega,
    Kilo,
    Hecto,
    Deca,
    Deci,
    Centi,
    Milli,
    Micro,
    Nano,
    Pico,
    Femto,
    Atto,
    Zepto,
    Yocto,
    Ronto,
    Quecto,
}

impl Prefix {
    pub const ALL: &'static [Prefix] = &[
        Prefix::Quetta,
        Prefix::Ronna,
        Prefix::Yotta,
        Prefix::Zetta,
        Prefix::Exa,
        Prefix::Peta,
        Prefix::Tera,
        Prefix::Giga,
        Prefix::Mega,
        Prefix::Kilo,
        Prefix::Hecto,
        Prefix::Deca,
        Prefix::Deci,
        Prefix::Centi,
        Prefix::Milli,
        Prefix::Micro,
        Prefix::Nano,
        Prefix::Pico,
        Prefix::Femto,
        Prefix::Atto,
        Prefix::Zepto,
        Prefix::Yocto,
        Prefix::Ronto,
        Prefix::Quecto,
    ];

    pub fn symbols(&self) -> &'static [&'static str] {
        match self {
            Prefix::Quetta => &["Q"],
            Prefix::Ronna => &["R"],
            Prefix::Yotta => &["Y"],
            Prefix::Zetta => &["Z"],
            Prefix::Exa => &["E"],
            Prefix::Peta => &["P"],
            Prefix::Tera => &["T"],
            Prefix::Giga => &["G"],
            Prefix::Mega => &["M"],
            Prefix::Kilo => &["k"],
            Prefix::Hecto => &["h"],
            Prefix::Deca => &["da"],
            Prefix::Deci => &["d"],
            Prefix::Centi => &["c"],
            Prefix::Milli => &["m"],
            Prefix::Micro => &["u", "\u{00b5}", "\u{03bc}"],
            Prefix::Nano => &["n"],
            Prefix::Pico => &["p"],
            Prefix::Femto => &["f"],
            Prefix::Atto => &["a"],
            Prefix::Zepto => &["z"],
            Prefix::Yocto => &["y"],
            Prefix::Ronto => &["r"],
            Prefix::Quecto => &["q"],
        }
    }

    pub fn names(&self) -> &'static [&'static str] {
        match self {
            Prefix::Quetta => &["quetta"],
            Prefix::Ronna => &["ronna"],
            Prefix::Yotta => &["yotta"],
            Prefix::Zetta => &["zetta"],
            Prefix::Exa => &["exa"],
            Prefix::Peta => &["peta"],
            Prefix::Tera => &["tera"],
            Prefix::Giga => &["giga"],
            Prefix::Mega => &["mega"],
            Prefix::Kilo => &["kilo"],
            Prefix::Hecto => &["hecto"],
            Prefix::Deca => &["deca", "deka"],
            Prefix::Deci => &["deci"],
            Prefix::Centi => &["centi"],
            Prefix::Milli => &["milli"],
            Prefix::Micro => &["micro"],
            Prefix::Nano => &["nano"],
            Prefix::Pico => &["pico"],
            Prefix::Femto => &["femto"],
            Prefix::Atto => &["atto"],
            Prefix::Zepto => &["zepto"],
            Prefix::Yocto => &["yocto"],
            Prefix::Ronto => &["ronto"],
            Prefix::Quecto => &["quecto"],
        }
    }

    pub fn symbol(&self) -> &'static str {
        self.symbols()[0]
    }

    pub fn name(&self) -> &'static str {
        self.names()[0]
    }

    pub fn exponent(&self) -> i32 {
        match self {
            Prefix::Quetta => 30,
            Prefix::Ronna => 27,
            Prefix::Yotta => 24,
            Prefix::Zetta => 21,
            Prefix::Exa => 18,
            Prefix::Peta => 15,
            Prefix::Tera => 12,
            Prefix::Giga => 9,
            Prefix::Mega => 6,
            Prefix::Kilo => 3,
            Prefix::Hecto => 2,
            Prefix::Deca => 1,
            Prefix::Deci => -1,
            Prefix::Centi => -2,
            Prefix::Milli => -3,
            Prefix::Micro => -6,
            Prefix::Nano => -9,
            Prefix::Pico => -12,
            Prefix::Femto => -15,
            Prefix::Atto => -18,
            Prefix::Zepto => -21,
            Prefix::Yocto => -24,
            Prefix::Ronto => -27,
            Prefix::Quecto => -30,
        }
    }

    pub fn factor(&self) -> Float {
        Float::parse(&format!("1e{}", self.exponent())).expect("power of ten parses")
    }
}

pub trait SiPrefixable: Copy + 'static {
    const PREFIXABLE: &'static [Self];

    fn symbol(&self) -> &'static str;

    fn names(&self) -> Vec<&'static str>;

    fn extra_symbol_forms(&self) -> &'static [&'static str] {
        &[]
    }

    fn symbol_forms(&self) -> Vec<&'static str> {
        let mut forms = vec![self.symbol()];
        forms.extend(self.extra_symbol_forms().iter().copied());
        forms.extend(
            self.names()
                .into_iter()
                .filter(|name| !is_spelled_out(name) && *name != self.symbol()),
        );
        forms
    }
}

fn is_spelled_out(name: &str) -> bool {
    name.len() >= 3 && name.chars().all(|c| c.is_ascii_lowercase())
}

pub fn parse_si_prefixed<T: SiPrefixable>(s: &str) -> Option<(Prefix, T)> {
    for prefix in Prefix::ALL {
        for symbol in prefix.symbols() {
            if let Some(rest) = s.strip_prefix(symbol)
                && let Some(unit) = T::PREFIXABLE
                    .iter()
                    .find(|unit| unit.symbol_forms().contains(&rest))
            {
                return Some((*prefix, *unit));
            }
        }
        for name in prefix.names() {
            if let Some(rest) = s.strip_prefix(name) {
                if !is_spelled_out(rest) {
                    continue;
                }
                if let Some(unit) = T::PREFIXABLE
                    .iter()
                    .find(|unit| unit.names().contains(&rest))
                {
                    return Some((*prefix, *unit));
                }
            }
        }
    }
    None
}
