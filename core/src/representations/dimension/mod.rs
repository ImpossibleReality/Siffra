mod absolute;
mod amount;
mod angle;
mod chemical;
mod currency;
mod current;
mod data;
mod derived;
mod length;
mod macros;
mod mass;
mod prefix;
mod temperature;
mod time;

use crate::representations::Float;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::ops::Neg;
use std::str::FromStr;

pub use {
    absolute::AbsoluteTemperature,
    amount::Amount,
    angle::Angle,
    chemical::{Compound, Element},
    currency::{Currency, install_static_rates},
    current::ElectricCurrent,
    data::Data,
    derived::DerivedUnit,
    length::Length,
    mass::Mass,
    prefix::{Prefix, SiPrefixable, parse_si_prefixed},
    temperature::TemperatureInterval,
    time::Time,
};

/// The abstract physical axes. Compatibility for addition, subtraction, and
/// conversion is decided *purely* here — never by which concrete unit is
/// shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BaseDimension {
    Length,
    Time,
    Mass,
    Amount,
    Angle,
    Temperature,
    Currency,
    Data,
    ElectricCurrent,
}

/// A physical dimension expressed as a map from base axis to exponent, e.g.
/// velocity is `{Length: 1, Time: -1}`. Derived from a [`Dimension`], so two
/// values
/// may be added/converted iff their dimensionalities are equal. Exponents are
/// [`Float`] so fractional powers (`sqrt(m^2) = m`) round-trip exactly.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Dimensionality(BTreeMap<BaseDimension, Float>);

impl Dimensionality {
    /// Adds `exponent` to `axis`, dropping the axis entirely when it cancels to
    /// zero so that equality ignores cancelled dimensions.
    pub fn add_axis(&mut self, axis: BaseDimension, exponent: Float) {
        let entry = self.0.entry(axis).or_insert_with(|| Float::from(0));
        *entry = &*entry + &exponent;
        if *entry == Float::from(0) {
            self.0.remove(&axis);
        }
    }

    /// True when this carries no physical dimension (a bare number, or a pure
    /// scalar unit such as `%`).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuantityKind {
    Length,
    Time,
    Mass,
    Amount,
    Angle,
    Temperature,
    TemperatureInterval,
    Data,
    ElectricCurrent,
}

/// A base unit: one of the primitive quantity axes, optionally tagged with a
/// chemical compound (for stoichiometric mass<->amount conversion).
#[derive(Debug, Clone, PartialEq)]
pub enum Quantity {
    Length(length::Length),
    Time(time::Time),
    Mass(mass::Mass, Option<Compound>),
    Amount(amount::Amount, Option<Compound>),
    Angle(angle::Angle),
    Temperature(temperature::TemperatureInterval),
    Data(data::Data),
    Current(current::ElectricCurrent),
}

impl FromStr for Quantity {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(length) = length::Length::from_str(s) {
            return Ok(Quantity::Length(length));
        }
        if let Ok(time) = time::Time::from_str(s) {
            return Ok(Quantity::Time(time));
        }
        if let Ok(mass) = mass::Mass::from_str(s) {
            return Ok(Quantity::Mass(mass, None));
        }
        if let Ok(amount) = amount::Amount::from_str(s) {
            return Ok(Quantity::Amount(amount, None));
        }
        if let Ok(angle) = angle::Angle::from_str(s) {
            return Ok(Quantity::Angle(angle));
        }
        if let Ok(temperature) = temperature::TemperatureInterval::from_str(s) {
            return Ok(Quantity::Temperature(temperature));
        }
        if let Ok(data) = data::Data::from_str(s) {
            return Ok(Quantity::Data(data));
        }
        if let Ok(current) = current::ElectricCurrent::from_str(s) {
            return Ok(Quantity::Current(current));
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
            Quantity::Temperature(_) => QuantityKind::Temperature,
            Quantity::Data(_) => QuantityKind::Data,
            Quantity::Current(_) => QuantityKind::ElectricCurrent,
        }
    }

    /// The abstract axis this base unit lives on.
    pub fn base_dimension(&self) -> BaseDimension {
        match self {
            Quantity::Length(_) => BaseDimension::Length,
            Quantity::Time(_) => BaseDimension::Time,
            Quantity::Mass(_, _) => BaseDimension::Mass,
            Quantity::Amount(_, _) => BaseDimension::Amount,
            Quantity::Angle(_) => BaseDimension::Angle,
            Quantity::Temperature(_) => BaseDimension::Temperature,
            Quantity::Data(_) => BaseDimension::Data,
            Quantity::Current(_) => BaseDimension::ElectricCurrent,
        }
    }

    pub fn shorthand(&self) -> String {
        match self {
            Quantity::Length(length) => length.shorthand().to_string(),
            Quantity::Time(time) => time.shorthand().to_string(),
            Quantity::Mass(mass, compound) => match compound {
                Some(compound) => mass.shorthand().to_string() + " " + &compound.to_string(),
                None => mass.shorthand().to_string(),
            },
            Quantity::Amount(amount, compound) => match compound {
                Some(compound) => amount.shorthand().to_string() + " " + &compound.to_string(),
                None => amount.shorthand().to_string(),
            },
            Quantity::Angle(angle) => angle.shorthand().to_string(),
            Quantity::Temperature(temperature) => temperature.shorthand().to_string(),
            Quantity::Data(data) => data.shorthand().to_string(),
            Quantity::Current(current) => current.shorthand().to_string(),
        }
    }

    /// Ratio to the canonical unit of this axis (meter, second, gram, mole,
    /// radian, kelvin).
    pub fn get_ratio(&self) -> Float {
        match self {
            Quantity::Length(length) => length.ratio(),
            Quantity::Time(time) => time.ratio(),
            Quantity::Mass(mass, _) => mass.ratio(),
            Quantity::Amount(amount, _) => amount.ratio(),
            Quantity::Angle(angle) => angle.ratio(),
            Quantity::Temperature(temperature) => temperature.ratio(),
            Quantity::Data(data) => data.ratio(),
            Quantity::Current(current) => current.ratio(),
        }
    }
}

/// A single named unit term: either a base [`Quantity`] or a named
/// [`DerivedUnit`]. This is what is *stored and displayed*; the physical
/// [`Dimensionality`] is derived from it, so `N·m` and `J` stay distinct on
/// screen even though they compare equal for arithmetic.
#[derive(Debug, Clone, PartialEq)]
pub enum Unit {
    Base(Quantity),
    Derived(DerivedUnit),
    /// An absolute (affine) temperature. Special-cased in conversion; decays to
    /// its interval unit under any arithmetic (see [`AbsoluteTemperature`]).
    Absolute(AbsoluteTemperature),
    /// A currency, whose conversion factor is a live exchange rate resolved at
    /// evaluation time (see [`Currency`]).
    Currency(Currency),
    /// A unit written with an SI prefix — `nF`, `daN`, `GWh`. Always wraps a
    /// [`Unit::Base`] or [`Unit::Derived`] whose family opted in via
    /// [`si_prefixable!`](crate::si_prefixable), so a prefixed unit costs no
    /// enum variant of its own (see [`prefix`]).
    Prefixed(Prefix, Box<Unit>),
}

impl Unit {
    pub fn base(quantity: Quantity) -> Self {
        Unit::Base(quantity)
    }

    /// Resolves a unit name, preferring base units and falling back to named
    /// derived units (`mph`, `N`, `J`, …).
    pub fn from_str(s: &str) -> Result<Self, ()> {
        if let Ok(quantity) = Quantity::from_str(s) {
            return Ok(Unit::Base(quantity));
        }
        // Resolve absolute temperatures first so `C`, `F`, and `K` stay
        // temperatures rather than becoming derived units.
        if let Ok(absolute) = AbsoluteTemperature::from_str(s) {
            return Ok(Unit::Absolute(absolute));
        }
        if let Ok(derived) = DerivedUnit::from_str(s) {
            return Ok(Unit::Derived(derived));
        }
        if let Ok(currency) = Currency::from_str(s) {
            return Ok(Unit::Currency(currency));
        }
        // Try prefixes only after exact names so established names such as
        // `pc`, `min`, and `cal` remain intact.
        if let Some(prefixed) = prefixed_from_str(s) {
            return Ok(prefixed);
        }
        Err(())
    }

    /// Attaches a chemical compound if this is a base mass/amount unit; a no-op
    /// otherwise.
    pub fn with_chemical(self, compound: Compound) -> Self {
        match self {
            Unit::Base(quantity) => Unit::Base(quantity.with_chemical(compound)),
            Unit::Prefixed(prefix, unit) => {
                Unit::Prefixed(prefix, Box::new(unit.with_chemical(compound)))
            }
            other => other,
        }
    }

    pub fn shorthand(&self) -> String {
        match self {
            Unit::Base(quantity) => quantity.shorthand(),
            Unit::Derived(derived) => derived.shorthand().to_string(),
            Unit::Absolute(absolute) => absolute.shorthand().to_string(),
            Unit::Currency(currency) => currency.code().to_string(),
            Unit::Prefixed(prefix, unit) => format!("{}{}", prefix.symbol(), unit.shorthand()),
        }
    }

    /// Splits any SI prefix off this unit: the multiplier the prefix
    /// contributes, and the unit it is written on. An unprefixed unit yields a
    /// multiplier of one and itself, so callers can treat both uniformly.
    fn split_prefix(&self) -> (Float, &Unit) {
        match self {
            Unit::Prefixed(prefix, unit) => (prefix.factor(), unit),
            other => (Float::from(1), other),
        }
    }

    /// The base-axis exponents contributed by *one* of this unit.
    pub fn dimensionality(&self) -> Dimensionality {
        let mut dim = Dimensionality::default();
        match self {
            Unit::Base(quantity) => dim.add_axis(quantity.base_dimension(), Float::from(1)),
            Unit::Derived(derived) => return derived.dimensionality(),
            Unit::Absolute(absolute) => return absolute.dimensionality(),
            Unit::Currency(_) => dim.add_axis(BaseDimension::Currency, Float::from(1)),
            Unit::Prefixed(_, unit) => return unit.dimensionality(),
        }
        dim
    }

    /// Conversion factor for *one* of this unit into canonical base units.
    ///
    /// Fallible because a currency's factor is a live exchange rate that may be
    /// unavailable. For an absolute temperature this is its *interval* (linear)
    /// scale — the affine offset is applied only by [`Value::convert`].
    pub fn factor(&self) -> Option<Float> {
        match self {
            Unit::Base(quantity) => Some(quantity.get_ratio()),
            Unit::Derived(derived) => Some(derived.factor()),
            Unit::Absolute(absolute) => {
                Some(Quantity::Temperature(absolute.interval()).get_ratio())
            }
            Unit::Currency(currency) => currency.factor(),
            Unit::Prefixed(prefix, unit) => Some(&prefix.factor() * &unit.factor()?),
        }
    }

    /// The underlying base quantity and the scale its SI prefix contributes, if
    /// this is a base unit (`mm` is the meter scaled by 1/1000).
    fn as_base(&self) -> Option<(&Quantity, Float)> {
        let (scale, unit) = self.split_prefix();
        match unit {
            Unit::Base(quantity) => Some((quantity, scale)),
            _ => None,
        }
    }
}

/// Resolves a unit name written as an SI prefix on a prefixable unit — `nF`,
/// `nanofarad`, `daN`, `Mm`. Base quantities are tried before named derived
/// units, mirroring the precedence of the exact lookups in
/// [`Unit::from_str`].
fn prefixed_from_str(s: &str) -> Option<Unit> {
    fn base<T: SiPrefixable>(s: &str, into_quantity: impl Fn(T) -> Quantity) -> Option<Unit> {
        let (prefix, unit) = parse_si_prefixed::<T>(s)?;
        Some(Unit::Prefixed(
            prefix,
            Box::new(Unit::Base(into_quantity(unit))),
        ))
    }

    base::<Length>(s, Quantity::Length)
        .or_else(|| base::<Time>(s, Quantity::Time))
        .or_else(|| base::<Mass>(s, |mass| Quantity::Mass(mass, None)))
        .or_else(|| base::<Amount>(s, |amount| Quantity::Amount(amount, None)))
        .or_else(|| base::<Angle>(s, Quantity::Angle))
        .or_else(|| base::<ElectricCurrent>(s, Quantity::Current))
        .or_else(|| {
            let (prefix, derived) = parse_si_prefixed::<DerivedUnit>(s)?;
            Some(Unit::Prefixed(prefix, Box::new(Unit::Derived(derived))))
        })
}

/// A stored unit expression: an ordered product of named unit terms with
/// (possibly fractional) powers, e.g. `mph` is `[(mph, 1)]` and `N·m` is
/// `[(N, 1), (m, 1)]`. This is the source of truth for rendering; math routes
/// through [`Dimension::dimensionality`].
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Dimension(pub Vec<(Unit, Float)>);

impl Display for Dimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut numerator = String::new();
        let mut denominator = String::new();

        for (unit, power) in self.0.iter() {
            let mut unit_shorthand = unit.shorthand();

            if *power > Float::from(0) {
                if !numerator.is_empty() {
                    unit_shorthand = format!("*{}", unit_shorthand);
                }
                numerator.push_str(&unit_shorthand);
                if *power != Float::from(1) {
                    numerator.push_str(&format!("^{}", power));
                }
            } else if *power < Float::from(0) {
                if !denominator.is_empty() {
                    unit_shorthand = format!("*{}", unit_shorthand);
                }
                denominator.push_str(&unit_shorthand);
                if *power != Float::from(-1) {
                    denominator.push_str(&format!("^{}", power.clone().neg()));
                }
            }
        }

        if !denominator.is_empty() {
            if numerator.is_empty() {
                write!(f, "1/{}", denominator)?;
            } else {
                write!(f, "{}/{}", numerator, denominator)?;
            }
        } else if !numerator.is_empty() {
            write!(f, "{}", numerator)?;
        }

        Ok(())
    }
}

impl Dimension {
    pub fn new(units: Vec<(Unit, Float)>) -> Self {
        Self(units).simplify()
    }

    /// The canonical percentage unit. Percentages deliberately remain named
    /// values instead of being eagerly divided by 100 so arithmetic can apply
    /// their contextual behavior (for example, `10 USD + 5%`).
    pub fn percent() -> Self {
        Self(vec![(Unit::Derived(DerivedUnit::Percent), Float::from(1))])
    }

    /// True only for a standalone percentage at power one. Other
    /// dimensionless scalar units (ppm, basis points, `%^2`, …) keep their
    /// ordinary unit-conversion behavior.
    pub fn is_percent(&self) -> bool {
        matches!(
            self.0.as_slice(),
            [(Unit::Derived(DerivedUnit::Percent), power)] if *power == Float::from(1)
        )
    }

    /// Merges identical unit terms, drops zero powers, and preserves different
    /// units on the same axis so `N·m` remains distinguishable from `J`.
    pub fn simplify(&self) -> Self {
        let mut new_dimension: Vec<(Unit, Float)> = Vec::new();

        for (unit, power) in self.0.iter() {
            let mut found = false;
            for (new_unit, new_power) in new_dimension.iter_mut() {
                if new_unit == unit {
                    *new_power = power + new_power;
                    found = true;
                    break;
                }
            }
            if !found {
                new_dimension.push((unit.clone(), power.clone()));
            }
        }

        new_dimension.retain(|(_, power)| *power != Float::from(0));

        Dimension(new_dimension)
    }

    /// The physical dimensionality of the whole expression (used for
    /// add/convert compatibility). Cancelled axes disappear, so `km*ms/h` and
    /// `m` both reduce to `{Length: 1}`.
    pub fn dimensionality(&self) -> Dimensionality {
        let mut dimensionality = Dimensionality::default();
        for (unit, power) in self.0.iter() {
            for (axis, exponent) in unit.dimensionality().0.iter() {
                dimensionality.add_axis(*axis, exponent * power);
            }
        }
        dimensionality
    }

    /// The conversion factor for the whole expression into canonical base
    /// units: the product of each term's factor raised to its power. `None` if
    /// any term's factor is unavailable (a currency with no live rate).
    pub fn factor(&self) -> Option<Float> {
        let mut factor = Float::from(1);
        for (unit, power) in self.0.iter() {
            factor = &factor * &unit.factor()?.pow(power);
        }
        Some(factor)
    }

    pub fn pow(&self, power: &Float) -> Self {
        let mut new_dimension = Vec::new();
        for (unit, old_power) in self.0.iter() {
            new_dimension.push((unit.clone(), old_power * power));
        }
        Dimension(new_dimension).simplify()
    }

    /// The multiplier taking a magnitude in `self`'s units to `other`'s units.
    /// It is `factor(other) / factor(self)` when dimensionalities match;
    /// returns `None` for incompatible dimensions, except the stoichiometric
    /// mass<->amount bridge.
    pub fn get_ratio(&self, other: &Self) -> Option<Float> {
        if self.dimensionality() == other.dimensionality() {
            return Some(&other.factor()? / &self.factor()?);
        }
        self.stoichiometric_ratio(other)
    }

    /// Handles the one conversion that crosses dimensionalities: a chemical
    /// compound weighed as mass on one side and counted as amount (moles) on
    /// the other, bridged by its molar mass. Base units only.
    fn stoichiometric_ratio(&self, other: &Self) -> Option<Float> {
        if self.0.len() != other.0.len() {
            return None;
        }

        let mut ratio = Float::from(1);
        let mut matched = vec![false; other.0.len()];

        for (unit, power) in self.0.iter() {
            let (quantity, scale) = unit.as_base()?;
            let mut match_index = None;

            for (index, (other_unit, other_power)) in other.0.iter().enumerate() {
                if matched[index] {
                    continue;
                }
                let (other_quantity, other_scale) = other_unit.as_base()?;
                if quantity.base_dimension() == other_quantity.base_dimension()
                    && *power == *other_power
                {
                    let mut quantity_ratio = &(&other_quantity.get_ratio() * &other_scale)
                        / &(&quantity.get_ratio() * &scale);
                    quantity_ratio = quantity_ratio.pow(power);
                    ratio = &ratio * &quantity_ratio;
                    match_index = Some(index);
                    break;
                }
            }

            if match_index.is_none() {
                for (index, (other_unit, other_power)) in other.0.iter().enumerate() {
                    if matched[index] {
                        continue;
                    }
                    let (other_quantity, other_scale) = other_unit.as_base()?;
                    if let Some(bridge_ratio) = Self::bridge_ratio(
                        (quantity, &scale),
                        power,
                        (other_quantity, &other_scale),
                        other_power,
                    ) {
                        ratio = &ratio * &bridge_ratio;
                        match_index = Some(index);
                        break;
                    }
                }
            }

            matched[match_index?] = true;
        }

        matched.iter().all(|used| *used).then_some(ratio)
    }

    /// Returns the multiplier for one legal mass<->amount chemical bridge.
    /// Both terms must be first power; all remaining terms are matched
    /// independently by [`stoichiometric_ratio`](Self::stoichiometric_ratio).
    /// Each side is a base quantity paired with the scale of any SI prefix
    /// written on it.
    fn bridge_ratio(
        (quantity, scale): (&Quantity, &Float),
        power: &Float,
        (other, other_scale): (&Quantity, &Float),
        other_power: &Float,
    ) -> Option<Float> {
        let one = Float::from(1);
        if *power != one || *other_power != one {
            return None;
        }

        match (quantity, other) {
            (Quantity::Mass(mass, Some(chemical)), Quantity::Amount(amount, other_chemical))
                if !matches!(other_chemical, Some(c) if c != chemical) =>
            {
                let amount_ratio = &Amount::Mole.ratio() / &(&amount.ratio() * other_scale);
                let mass_ratio = &Mass::Gram.ratio() / &(&mass.ratio() * scale);
                Some(&(&mass_ratio / &amount_ratio) * &chemical.particulate_mass())
            }
            (Quantity::Amount(amount, Some(chemical)), Quantity::Mass(mass, other_chemical))
                if !matches!(other_chemical, Some(c) if c != chemical) =>
            {
                let amount_ratio = &Amount::Mole.ratio() / &(&amount.ratio() * scale);
                let mass_ratio = &Mass::Gram.ratio() / &(&mass.ratio() * other_scale);
                Some(&(&amount_ratio / &mass_ratio) / &chemical.particulate_mass())
            }
            _ => None,
        }
    }

    pub fn is_unitless(&self) -> bool {
        self.0.is_empty()
    }

    /// The absolute temperature iff this expression is exactly one such unit at
    /// power +1, the only shape for which an affine transform is meaningful.
    /// Anything else (compound, or a power other than 1) is not an absolute
    /// temperature and converts linearly.
    pub fn as_absolute_temperature(&self) -> Option<AbsoluteTemperature> {
        match self.0.as_slice() {
            [(Unit::Absolute(absolute), power)] if *power == Float::from(1) => Some(*absolute),
            _ => None,
        }
    }

    /// Decays any absolute-temperature term to its linear interval unit unless
    /// this is a standalone power-1 absolute temperature. Called after
    /// multiplication or exponentiation, where the affine identity no longer
    /// holds.
    /// Factor-preserving, so the associated magnitude does not change.
    pub fn decay_absolute(self) -> Self {
        if self.as_absolute_temperature().is_some() {
            return self;
        }
        let terms = self
            .0
            .into_iter()
            .map(|(unit, power)| match unit {
                Unit::Absolute(absolute) => (
                    Unit::Base(Quantity::Temperature(absolute.interval())),
                    power,
                ),
                other => (other, power),
            })
            .collect();
        Dimension(terms)
    }

    /// Reverse-lookup index: every named derived unit whose dimensionality
    /// equals this expression's — the candidates for a named collapse (e.g.
    /// `kg*m/s^2`
    /// yields `[Newton]`). Exposed for an explicit `simplify`; not applied
    /// automatically.
    pub fn named_equivalents(&self) -> Vec<DerivedUnit> {
        let dimensionality = self.dimensionality();
        DerivedUnit::ALL
            .iter()
            .copied()
            .filter(|derived| derived.dimensionality() == dimensionality)
            .collect()
    }

    /// Expands every named [`DerivedUnit`] term into the product of base
    /// [`Quantity`] units it is defined from (`N` -> `kg*m/s^2`, `mph` -> `mi/h`,
    /// `%` -> dimensionless), distributing the term's power over the expansion
    /// and leaving base, currency and absolute-temperature terms untouched.
    ///
    /// The result has the same physical [`Dimensionality`], so a [`Value`] can
    /// be `convert`ed into it losslessly — any scalar factor a derived unit
    /// carried (the 3.6M in `kWh`, the 1/100 in `%`) is absorbed into the
    /// converted magnitude. Inverse of [`collapse_to_named`](Self::collapse_to_named);
    /// backs an explicit `expand` command.
    pub fn expand(&self) -> Dimension {
        let mut terms: Vec<(Unit, Float)> = Vec::new();
        for (unit, power) in self.0.iter() {
            match unit.split_prefix().1 {
                Unit::Derived(derived) => {
                    for (quantity, base_power) in derived.expansion() {
                        terms.push((Unit::Base(quantity), &Float::from(base_power) * power));
                    }
                }
                _ => terms.push((unit.clone(), power.clone())),
            }
        }
        Dimension(terms).simplify()
    }

    /// If exactly one named unit matches this whole expression's
    /// dimensionality,
    /// returns that single-term expression (`kg*m/s^2` -> `N`). Ready for an
    /// explicit `simplify` command; ambiguous matches (none / several) yield
    /// `None`.
    pub fn collapse_to_named(&self) -> Option<Dimension> {
        let canonical: Vec<DerivedUnit> = self
            .named_equivalents()
            .into_iter()
            .filter(DerivedUnit::is_named_canonical)
            .collect();
        match canonical.as_slice() {
            [derived] => Some(Dimension(vec![(Unit::Derived(*derived), Float::from(1))])),
            _ => None,
        }
    }

    /// When multiplying `self` by `other`, returns a clean target expression
    /// iff
    /// the product *fully cancels* one or more base axes across differing units
    /// (e.g. `mph * s` cancels Time, `J / N` cancels Mass and Time). The target
    /// is built from the operands' own units, so `mph * s` collapses to miles
    /// rather than showing a mixed `mph*s`.
    ///
    /// Deliberately conservative: an axis must vanish entirely. Products where
    /// every axis survives (`N * m`, `kg * m/s^2`) return `None` and keep their
    /// display, so this never editorialises a clean product into a named unit.
    pub fn collapse_target(&self, other: &Self) -> Option<Dimension> {
        let self_dim = self.dimensionality();
        let other_dim = other.dimensionality();

        let fully_cancels = self_dim.0.iter().any(|(axis, exponent)| {
            other_dim
                .0
                .get(axis)
                .is_some_and(|other_exponent| (exponent + other_exponent) == Float::from(0))
        });
        if !fully_cancels {
            return None;
        }

        let mut combined = self_dim;
        for (axis, exponent) in other_dim.0.iter() {
            combined.add_axis(*axis, exponent.clone());
        }
        if combined.is_empty() {
            return Some(Dimension::default());
        }

        // Prefer units named by derived operands, then incidental units, then
        // the canonical unit for the surviving axis.
        let mut preferred: BTreeMap<BaseDimension, Unit> = BTreeMap::new();
        let mut incidental: BTreeMap<BaseDimension, Unit> = BTreeMap::new();
        for (unit, _) in self.0.iter().chain(other.0.iter()) {
            match unit.split_prefix().1 {
                Unit::Base(quantity) => {
                    incidental
                        .entry(quantity.base_dimension())
                        .or_insert_with(|| unit.clone());
                }
                Unit::Absolute(absolute) => {
                    incidental
                        .entry(BaseDimension::Temperature)
                        .or_insert_with(|| Unit::Base(Quantity::Temperature(absolute.interval())));
                }
                Unit::Currency(_) => {
                    incidental
                        .entry(BaseDimension::Currency)
                        .or_insert_with(|| unit.clone());
                }
                Unit::Derived(derived) => {
                    for (quantity, _) in derived.expansion() {
                        preferred
                            .entry(quantity.base_dimension())
                            .or_insert_with(|| Unit::Base(quantity.clone()));
                    }
                }
                Unit::Prefixed(..) => {}
            }
        }

        let mut target = Vec::new();
        for (axis, exponent) in combined.0.iter() {
            let unit = preferred
                .get(axis)
                .or_else(|| incidental.get(axis))
                .cloned()
                .unwrap_or_else(|| default_unit(*axis));
            target.push((unit, exponent.clone()));
        }
        Some(Dimension(target))
    }
}

/// The canonical unit for each axis — the last-resort collapse target when no
/// operand names a surviving axis (EUR for currency, otherwise the ratio-1 base
/// unit).
fn default_unit(axis: BaseDimension) -> Unit {
    match axis {
        BaseDimension::Length => Unit::Base(Quantity::Length(Length::Meter)),
        BaseDimension::Time => Unit::Base(Quantity::Time(Time::Second)),
        BaseDimension::Mass => Unit::Base(Quantity::Mass(Mass::Gram, None)),
        BaseDimension::Amount => Unit::Base(Quantity::Amount(Amount::Mole, None)),
        BaseDimension::Angle => Unit::Base(Quantity::Angle(Angle::Radian)),
        BaseDimension::Temperature => {
            Unit::Base(Quantity::Temperature(TemperatureInterval::Kelvin))
        }
        BaseDimension::Currency => Unit::Currency(Currency::canonical()),
        BaseDimension::Data => Unit::Base(Quantity::Data(Data::Bit)),
        BaseDimension::ElectricCurrent => Unit::Base(Quantity::Current(ElectricCurrent::Ampere)),
    }
}
