//! SI decimal prefixes, and the opt-in mechanism that lets any unit family use
//! them.
//!
//! A prefixed unit is stored alongside the unit it modifies
//! (see [`Unit::Prefixed`]) rather than as a separate enum variant. A family
//! opts in by naming its *coherent* member(s) with [`si_prefixable!`]:
//!
//! ```ignore
//! si_prefixable!(Length, [Meter]);
//! ```
//!
//! from which `Zm`, `daL`, `quettameters` and the rest follow automatically.
//!
//! Two rules keep this from swallowing established names:
//!
//! 1. A prefix is only tried *after* every exact unit name has failed to match
//!    ([`Unit::from_str`]), so `pc` stays a parsec (not a picocoulomb), `min` a
//!    minute (not a milli-inch) and `cal` a calorie.
//! 2. A symbol prefix only attaches to a unit's symbol and a spelled-out prefix
//!    only to its spelled-out names — `nF` and `nanofarad` resolve, the mixed
//!    `nfarad` and `nanoF` do not.
//!
//! [`Unit::Prefixed`]: crate::representations::Unit::Prefixed
//! [`Unit::from_str`]: crate::representations::Unit::from_str

use crate::representations::Float;

/// An SI decimal prefix: a power of ten that scales the unit it is written on.
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
    /// Every SI prefix, largest first. Resolution walks this order, which also
    /// settles the one ambiguous split: `da` (deca) is tried before `d` (deci),
    /// so `daN` is ten newtons rather than a failed deci-`aN`.
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

    /// The symbols this prefix is written with, canonical form first. Micro
    /// also accepts the two Greek mu characters; `u` remains the usual ASCII
    /// form.
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

    /// The spelled-out forms of this prefix, canonical first.
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

    /// The canonical symbol, used for display (`nF`, `daN`).
    pub fn symbol(&self) -> &'static str {
        self.symbols()[0]
    }

    /// The canonical spelled-out name.
    pub fn name(&self) -> &'static str {
        self.names()[0]
    }

    /// The power of ten this prefix stands for.
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

    /// The multiplier this prefix contributes to a unit's conversion factor.
    pub fn factor(&self) -> Float {
        Float::parse(&format!("1e{}", self.exponent())).expect("power of ten parses")
    }
}

/// A family of units that SI prefixes may be written on.
///
/// Implemented with [`si_prefixable!`], which lists the *coherent* members of
/// the family — the ones a prefix actually means something on (`Meter`, never
/// `Kilometer` or `Mile`). Non-SI units that are conventionally prefixed anyway
/// (the liter, the bar, the tonne) are listed too, since that is what people
/// write.
pub trait SiPrefixable: Copy + 'static {
    /// The units of this family a prefix may attach to.
    const PREFIXABLE: &'static [Self];

    /// The unit's canonical symbol — the display form a prefix is written on.
    fn symbol(&self) -> &'static str;

    /// Every name the unit parses from, symbols and spelled-out forms alike.
    fn names(&self) -> Vec<&'static str>;

    /// Abbreviation-style aliases a symbol prefix may sit on even though they
    /// look like words, declared per unit for the spellings convention mixes
    /// (`msec`, `kgmol`).
    fn extra_symbol_forms(&self) -> &'static [&'static str] {
        &[]
    }

    /// The names a *symbol* prefix may attach to: the canonical symbol, the
    /// declared abbreviations, and any alias that is not a spelled-out word.
    /// That last group is what lets `MOhm` read like `Mohm` and `kl` like `kL`,
    /// without admitting `nanoF`.
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

/// Whether `name` is a spelled-out unit name rather than a symbol, and so may
/// carry a spelled-out prefix. Three or more lowercase ASCII letters: enough to
/// admit `farad`, `liters` and `ohm` while rejecting the symbol-ish aliases
/// that share the same list (`L`, `l`, `Da`), which would otherwise let
/// `millil` through.
fn is_spelled_out(name: &str) -> bool {
    name.len() >= 3 && name.chars().all(|c| c.is_ascii_lowercase())
}

/// Resolves `s` as a prefix written on a unit of family `T` — `nF`,
/// `nanofarad`, `nanofarads` — or `None` when it is not one.
///
/// Symbol prefixes pair with symbols and spelled-out prefixes with spelled-out
/// names, so the halves of a name always agree in style.
pub fn parse_si_prefixed<T: SiPrefixable>(s: &str) -> Option<(Prefix, T)> {
    for prefix in Prefix::ALL {
        for symbol in prefix.symbols() {
            if let Some(rest) = s.strip_prefix(symbol) {
                if let Some(unit) = T::PREFIXABLE
                    .iter()
                    .find(|unit| unit.symbol_forms().contains(&rest))
                {
                    return Some((*prefix, *unit));
                }
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
