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
mod timezone;

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
    timezone::TimeZone,
};

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

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Dimensionality(BTreeMap<BaseDimension, Float>);

impl Dimensionality {
    pub fn add_axis(&mut self, axis: BaseDimension, exponent: Float) {
        let entry = self.0.entry(axis).or_insert_with(|| Float::from(0));
        *entry = &*entry + &exponent;
        if *entry == Float::from(0) {
            self.0.remove(&axis);
        }
    }

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

#[derive(Debug, Clone, PartialEq)]
pub enum Unit {
    Base(Quantity),
    Derived(DerivedUnit),
    Absolute(AbsoluteTemperature),
    /// An affine point on the Time axis, measured in seconds since the Unix
    /// epoch. The scalar is zone-independent, so changing zone is a relabel.
    Instant(TimeZone),
    /// A wall-clock reading. Stored the same way an instant is, on a date that
    /// only exists to resolve the zone's offset, so the two share their
    /// arithmetic and differ in what they show.
    Clock(TimeZone),
    Currency(Currency),
    Prefixed(Prefix, Box<Unit>),
}

/// The affine units: points on an axis rather than lengths along it. Arithmetic
/// other than "point ± interval" and "point − point" decays them to intervals.
#[derive(Debug, Clone, PartialEq)]
pub enum Affine {
    Temperature(AbsoluteTemperature),
    Instant(TimeZone),
    Clock(TimeZone),
}

impl Affine {
    /// The zone a temporal point is read in, if this is one.
    pub fn zone(&self) -> Option<&TimeZone> {
        match self {
            Affine::Instant(zone) | Affine::Clock(zone) => Some(zone),
            Affine::Temperature(_) => None,
        }
    }

    pub fn is_clock(&self) -> bool {
        matches!(self, Affine::Clock(_))
    }
}

impl Unit {
    pub fn base(quantity: Quantity) -> Self {
        Unit::Base(quantity)
    }

    pub fn from_str(s: &str) -> Result<Self, ()> {
        if let Ok(quantity) = Quantity::from_str(s) {
            return Ok(Unit::Base(quantity));
        }
        if let Ok(absolute) = AbsoluteTemperature::from_str(s) {
            return Ok(Unit::Absolute(absolute));
        }
        if let Ok(derived) = DerivedUnit::from_str(s) {
            return Ok(Unit::Derived(derived));
        }
        if let Ok(currency) = Currency::from_str(s) {
            return Ok(Unit::Currency(currency));
        }
        if let Some(prefixed) = prefixed_from_str(s) {
            return Ok(prefixed);
        }
        if let Some(zone) = TimeZone::parse(s) {
            return Ok(Unit::Instant(zone));
        }
        Err(())
    }

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
            Unit::Instant(zone) | Unit::Clock(zone) => zone.label().to_string(),
            Unit::Currency(currency) => currency.code().to_string(),
            Unit::Prefixed(prefix, unit) => format!("{}{}", prefix.symbol(), unit.shorthand()),
        }
    }

    fn split_prefix(&self) -> (Float, &Unit) {
        match self {
            Unit::Prefixed(prefix, unit) => (prefix.factor(), unit),
            other => (Float::from(1), other),
        }
    }

    pub fn dimensionality(&self) -> Dimensionality {
        let mut dim = Dimensionality::default();
        match self {
            Unit::Base(quantity) => dim.add_axis(quantity.base_dimension(), Float::from(1)),
            Unit::Derived(derived) => return derived.dimensionality(),
            Unit::Absolute(absolute) => return absolute.dimensionality(),
            Unit::Instant(_) | Unit::Clock(_) => dim.add_axis(BaseDimension::Time, Float::from(1)),
            Unit::Currency(_) => dim.add_axis(BaseDimension::Currency, Float::from(1)),
            Unit::Prefixed(_, unit) => return unit.dimensionality(),
        }
        dim
    }

    pub fn factor(&self) -> Option<Float> {
        match self {
            Unit::Base(quantity) => Some(quantity.get_ratio()),
            Unit::Derived(derived) => Some(derived.factor()),
            Unit::Absolute(absolute) => {
                Some(Quantity::Temperature(absolute.interval()).get_ratio())
            }
            // Epoch seconds; the zone offsets the reading, never the scalar.
            Unit::Instant(_) | Unit::Clock(_) => Some(Float::from(1)),
            Unit::Currency(currency) => currency.factor(),
            Unit::Prefixed(prefix, unit) => Some(&prefix.factor() * &unit.factor()?),
        }
    }

    fn as_base(&self) -> Option<(&Quantity, Float)> {
        let (scale, unit) = self.split_prefix();
        match unit {
            Unit::Base(quantity) => Some((quantity, scale)),
            _ => None,
        }
    }
}

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

/// An ordered product of named unit terms with (possibly fractional) powers
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

    pub fn percent() -> Self {
        Self(vec![(Unit::Derived(DerivedUnit::Percent), Float::from(1))])
    }

    pub fn is_percent(&self) -> bool {
        matches!(
            self.0.as_slice(),
            [(Unit::Derived(DerivedUnit::Percent), power)] if *power == Float::from(1)
        )
    }

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

    pub fn dimensionality(&self) -> Dimensionality {
        let mut dimensionality = Dimensionality::default();
        for (unit, power) in self.0.iter() {
            for (axis, exponent) in unit.dimensionality().0.iter() {
                dimensionality.add_axis(*axis, exponent * power);
            }
        }
        dimensionality
    }

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

    /// Returns the multiplier that converts `self` into `other`.
    pub fn get_ratio(&self, other: &Self) -> Option<Float> {
        if self.dimensionality() == other.dimensionality() {
            return Some(&other.factor()? / &self.factor()?);
        }
        self.stoichiometric_ratio(other)
    }

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

    fn bridge_ratio(
        (quantity, scale): (&Quantity, &Float),
        power: &Float,
        (other, other_scale): (&Quantity, &Float),
        other_power: &Float,
    ) -> Option<Float> {
        // Stoichiometric bridges are valid only for first-power terms.
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

    /// Returns the lone affine term, if this dimension is exactly one.
    pub fn as_affine(&self) -> Option<Affine> {
        match self.0.as_slice() {
            [(Unit::Absolute(absolute), power)] if *power == Float::from(1) => {
                Some(Affine::Temperature(*absolute))
            }
            [(Unit::Instant(zone), power)] if *power == Float::from(1) => {
                Some(Affine::Instant(zone.clone()))
            }
            [(Unit::Clock(zone), power)] if *power == Float::from(1) => {
                Some(Affine::Clock(zone.clone()))
            }
            _ => None,
        }
    }

    /// The lone temporal point this dimension names, if it is one.
    pub fn as_temporal_point(&self) -> Option<Affine> {
        self.as_affine().filter(|affine| affine.zone().is_some())
    }

    pub fn is_temporal_point(&self) -> bool {
        self.as_temporal_point().is_some()
    }

    pub fn as_absolute_temperature(&self) -> Option<AbsoluteTemperature> {
        match self.as_affine() {
            Some(Affine::Temperature(absolute)) => Some(absolute),
            _ => None,
        }
    }

    pub fn as_instant(&self) -> Option<TimeZone> {
        match self.as_affine() {
            Some(Affine::Instant(zone)) => Some(zone),
            _ => None,
        }
    }

    pub fn as_clock(&self) -> Option<TimeZone> {
        match self.as_affine() {
            Some(Affine::Clock(zone)) => Some(zone),
            _ => None,
        }
    }

    /// Replaces affine units with their linear counterparts after arithmetic.
    ///
    /// A scaled temperature is still a temperature, so a lone one survives.
    /// A scaled instant is not a date — there is no calendar meaning to twice
    /// the epoch — so instants and clocks always decay to elapsed seconds.
    pub fn decay_affine(self) -> Self {
        let keep_temperature = self.as_absolute_temperature().is_some();
        let terms = self
            .0
            .into_iter()
            .map(|(unit, power)| match unit {
                Unit::Absolute(absolute) if !keep_temperature => (
                    Unit::Base(Quantity::Temperature(absolute.interval())),
                    power,
                ),
                Unit::Instant(_) | Unit::Clock(_) => {
                    (Unit::Base(Quantity::Time(Time::Second)), power)
                }
                other => (other, power),
            })
            .collect();
        Dimension(terms)
    }

    /// The lone calendar-length term this dimension names, if any. These are
    /// the irregular civil units whose length depends on where on the calendar
    /// they land; hour and below are exact SI multiples and stay on the ratio
    /// path.
    pub fn as_calendar_unit(&self) -> Option<Time> {
        match self.0.as_slice() {
            [(Unit::Base(Quantity::Time(unit)), power)] if *power == Float::from(1) => {
                is_calendar_unit(*unit).then_some(*unit)
            }
            _ => None,
        }
    }

    /// Whether this dimension is a plain time interval of the first power.
    pub fn is_duration(&self) -> bool {
        let mut axes = self.dimensionality().0.into_iter();
        matches!(
            (axes.next(), axes.next()),
            (Some((BaseDimension::Time, exponent)), None) if exponent == Float::from(1)
        ) && !self.is_temporal_point()
    }

    pub fn named_equivalents(&self) -> Vec<DerivedUnit> {
        let dimensionality = self.dimensionality();
        DerivedUnit::ALL
            .iter()
            .copied()
            .filter(|derived| derived.dimensionality() == dimensionality)
            .collect()
    }

    /// Expands named derived units into base units while preserving factors.
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

    /// Collapses to a unique canonical named unit, if one exists.
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

    /// Chooses a display dimension when multiplying operands cancels an axis.
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
                Unit::Instant(_) | Unit::Clock(_) => {
                    incidental
                        .entry(BaseDimension::Time)
                        .or_insert_with(|| Unit::Base(Quantity::Time(Time::Second)));
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

/// Day and larger are civil units: DST, month lengths and leap years make their
/// length depend on the calendar. Hour and smaller are exact SI multiples.
pub fn is_calendar_unit(unit: Time) -> bool {
    matches!(
        unit,
        Time::Day
            | Time::Week
            | Time::Fortnight
            | Time::Month
            | Time::Year
            | Time::Decade
            | Time::Century
            | Time::Millennium
    )
}

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
