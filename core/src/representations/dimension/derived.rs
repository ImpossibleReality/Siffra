use crate::representations::dimension::{
    Angle, Data, Dimensionality, ElectricCurrent, Length, Mass, Quantity, Time,
};
use crate::representations::Float;
use crate::{ratio, si_prefixable};
use std::str::FromStr;

/// A named *derived* unit: a single token (`N`, `mph`, `%`, `kWh`, `V`) that
/// stands for a dimensionless scalar times a product of base units.
///
/// The whole point of storing it as its own term — rather than eagerly
/// expanding it into base units — is *display*: `N·m` (torque) and `J`
/// (energy) share a dimensionality but must render differently, so the named
/// unit must be retained.
///
/// Both the [`Dimensionality`] and the canonical conversion factor are computed
/// from [`DerivedUnit::expansion`] and [`DerivedUnit::scalar`], so a derived
/// unit stays synchronized with the base units it is built from.
///
/// The table below is the single source of truth. Each entry lists the display
/// `short`/`long` names, extra parse `aka` aliases, the dimensionless `scalar`,
/// and the `base` expansion (base [`Quantity`]s with integer powers).
///
/// The conversion factor is `scalar * Π base_i.ratio ^ power_i`, expressed in
/// canonical base units (g, m, s, mol, rad, K, A, bit). So a unit that is a
/// messy multiple of base units — a liter, a calorie, or a psi — is just a
/// scalar on a clean base expansion.
macro_rules! derived_units {
    ($( $variant:ident {
        short: $short:expr,
        long: $long:expr,
        aka: [ $($aka:expr),* $(,)? ],
        scalar: $scalar:expr,
        base: [ $( ($q:expr, $p:expr) ),* $(,)? ] $(,)?
    } ),* $(,)? ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum DerivedUnit {
            $($variant),*
        }

        impl DerivedUnit {
            /// Every named derived unit. Backs name resolution and the
            /// reverse-lookup index
            /// ([`crate::representations::Dimension::named_equivalents`]).
            pub const ALL: &'static [DerivedUnit] = &[ $(DerivedUnit::$variant),* ];

            /// The product of base [`Quantity`]s this unit expands to, with
            /// integer powers. Empty for pure scalars such as `percent` or
            /// `ppm`.
            pub fn expansion(&self) -> Vec<(Quantity, i32)> {
                match self {
                    $( DerivedUnit::$variant => vec![ $( ($q, $p) ),* ] ),*
                }
            }

            /// The dimensionless multiplier folded into the conversion factor:
            /// `1` for a clean SI-derived unit (N, J, V), a fraction for `%`,
            /// or a messy constant for a liter / calorie / psi.
            fn scalar(&self) -> Float {
                match self {
                    $( DerivedUnit::$variant => $scalar ),*
                }
            }

            pub fn shorthand(&self) -> &'static str {
                match self {
                    $( DerivedUnit::$variant => $short ),*
                }
            }

            pub fn longhand(&self) -> &'static str {
                match self {
                    $( DerivedUnit::$variant => $long ),*
                }
            }

            /// All names this unit parses from, including extra aliases.
            pub fn aliases(&self) -> Vec<&'static str> {
                match self {
                    $( DerivedUnit::$variant => vec![$short, $long, $($aka),*] ),*
                }
            }
        }
    };
}

derived_units! {
    Newton { short: "N", long: "newton", aka: ["newtons"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 1), (Quantity::Time(Time::Second), -2)] },
    Joule { short: "J", long: "joule", aka: ["joules"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },
    Watt { short: "W", long: "watt", aka: ["watts"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -3)] },
    Pascal { short: "Pa", long: "pascal", aka: ["pascals"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), -1), (Quantity::Time(Time::Second), -2)] },
    Hertz { short: "Hz", long: "hertz", aka: [], scalar: ratio!(1 / 1),
        base: [(Quantity::Time(Time::Second), -1)] },

    Dyne { short: "dyn", long: "dyne", aka: ["dynes"], scalar: ratio!(1 / 100_000),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 1), (Quantity::Time(Time::Second), -2)] },
    PoundForce { short: "lbf", long: "pound-force", aka: ["poundforce"], scalar: Float::parse("4.4482216152605").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 1), (Quantity::Time(Time::Second), -2)] },
    KilogramForce { short: "kgf", long: "kilogram-force", aka: ["kilogramforce", "kp"], scalar: Float::parse("9.80665").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 1), (Quantity::Time(Time::Second), -2)] },

    WattHour { short: "Wh", long: "watt-hour", aka: ["watthour", "watthours"], scalar: ratio!(3_600 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },
    Calorie { short: "cal", long: "calorie", aka: ["calories"], scalar: Float::parse("4.184").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },
    Kilocalorie { short: "kcal", long: "kilocalorie", aka: ["kilocalories", "Calorie", "Calories"], scalar: ratio!(4_184 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },
    Btu { short: "BTU", long: "british thermal unit", aka: ["btu"], scalar: Float::parse("1055.05585262").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },
    Electronvolt { short: "eV", long: "electronvolt", aka: ["electronvolts"], scalar: Float::parse("1.602176634e-19").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },
    Erg { short: "erg", long: "erg", aka: ["ergs"], scalar: ratio!(1 / 10_000_000),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },
    Therm { short: "therm", long: "therm", aka: ["therms"], scalar: Float::parse("105480400").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },
    FootPound { short: "ftlb", long: "foot-pound", aka: ["footpound", "footpounds", "ftlbf"], scalar: Float::parse("1.3558179483314004").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2)] },

    Horsepower { short: "hp", long: "horsepower", aka: ["hps"], scalar: Float::parse("745.69987158227022").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -3)] },
    MetricHorsepower { short: "PS", long: "metric horsepower", aka: ["cv", "ch"], scalar: Float::parse("735.49875").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -3)] },

    Bar { short: "bar", long: "bar", aka: ["bars"], scalar: ratio!(100_000 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), -1), (Quantity::Time(Time::Second), -2)] },
    Atmosphere { short: "atm", long: "atmosphere", aka: ["atmospheres"], scalar: ratio!(101_325 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), -1), (Quantity::Time(Time::Second), -2)] },
    Psi { short: "psi", long: "pound per square inch", aka: [], scalar: Float::parse("6894.757293168361").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), -1), (Quantity::Time(Time::Second), -2)] },
    Torr { short: "torr", long: "torr", aka: [], scalar: ratio!(101_325 / 760),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), -1), (Quantity::Time(Time::Second), -2)] },
    MillimeterMercury { short: "mmHg", long: "millimeter of mercury", aka: [], scalar: Float::parse("133.322387415").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), -1), (Quantity::Time(Time::Second), -2)] },
    InchMercury { short: "inHg", long: "inch of mercury", aka: [], scalar: Float::parse("3386.389").unwrap(),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), -1), (Quantity::Time(Time::Second), -2)] },

    Rpm { short: "rpm", long: "revolutions per minute", aka: [], scalar: ratio!(1 / 1),
        base: [(Quantity::Angle(Angle::Revolution), 1), (Quantity::Time(Time::Minute), -1)] },

    MilePerHour { short: "mph", long: "mile per hour", aka: [], scalar: ratio!(1 / 1),
        base: [(Quantity::Length(Length::Mile), 1), (Quantity::Time(Time::Hour), -1)] },
    KilometerPerHour { short: "kph", long: "kilometer per hour", aka: ["kmh"], scalar: ratio!(1 / 1),
        base: [(Quantity::Length(Length::Kilometer), 1), (Quantity::Time(Time::Hour), -1)] },
    FootPerSecond { short: "fps", long: "foot per second", aka: [], scalar: ratio!(1 / 1),
        base: [(Quantity::Length(Length::Foot), 1), (Quantity::Time(Time::Second), -1)] },
    Knot { short: "kn", long: "knot", aka: ["kt", "knots"], scalar: ratio!(1 / 1),
        base: [(Quantity::Length(Length::NauticalMile), 1), (Quantity::Time(Time::Hour), -1)] },
    Mach { short: "Mach", long: "mach", aka: ["mach"], scalar: Float::parse("340.29").unwrap(),
        base: [(Quantity::Length(Length::Meter), 1), (Quantity::Time(Time::Second), -1)] },

    Hectare { short: "ha", long: "hectare", aka: ["hectares"], scalar: ratio!(10_000 / 1),
        base: [(Quantity::Length(Length::Meter), 2)] },
    Acre { short: "acre", long: "acre", aka: ["acres"], scalar: Float::parse("4046.8564224").unwrap(),
        base: [(Quantity::Length(Length::Meter), 2)] },
    Are { short: "are", long: "are", aka: ["ares"], scalar: ratio!(100 / 1),
        base: [(Quantity::Length(Length::Meter), 2)] },

    Liter { short: "L", long: "liter", aka: ["litre", "liters", "litres", "l"], scalar: ratio!(1 / 1_000),
        base: [(Quantity::Length(Length::Meter), 3)] },
    Gallon { short: "gal", long: "gallon", aka: ["gallons"], scalar: Float::parse("0.003785411784").unwrap(),
        base: [(Quantity::Length(Length::Meter), 3)] },
    Quart { short: "qt", long: "quart", aka: ["quarts"], scalar: Float::parse("0.000946352946").unwrap(),
        base: [(Quantity::Length(Length::Meter), 3)] },
    Pint { short: "pt", long: "pint", aka: ["pints"], scalar: Float::parse("0.000473176473").unwrap(),
        base: [(Quantity::Length(Length::Meter), 3)] },
    Cup { short: "cup", long: "cup", aka: ["cups"], scalar: Float::parse("0.0002365882365").unwrap(),
        base: [(Quantity::Length(Length::Meter), 3)] },
    FluidOunce { short: "floz", long: "fluid ounce", aka: ["fluidounce", "fluidounces"], scalar: Float::parse("0.0000295735295625").unwrap(),
        base: [(Quantity::Length(Length::Meter), 3)] },
    Tablespoon { short: "tbsp", long: "tablespoon", aka: ["tablespoons", "tbsps"], scalar: Float::parse("0.00001478676478125").unwrap(),
        base: [(Quantity::Length(Length::Meter), 3)] },
    Teaspoon { short: "tsp", long: "teaspoon", aka: ["teaspoons", "tsps"], scalar: Float::parse("0.00000492892159375").unwrap(),
        base: [(Quantity::Length(Length::Meter), 3)] },
    Barrel { short: "bbl", long: "barrel", aka: ["barrels"], scalar: Float::parse("0.158987294928").unwrap(),
        base: [(Quantity::Length(Length::Meter), 3)] },

    BitPerSecond { short: "bps", long: "bit per second", aka: [], scalar: ratio!(1 / 1),
        base: [(Quantity::Data(Data::Bit), 1), (Quantity::Time(Time::Second), -1)] },
    KilobitPerSecond { short: "kbps", long: "kilobit per second", aka: [], scalar: ratio!(1 / 1),
        base: [(Quantity::Data(Data::Kilobit), 1), (Quantity::Time(Time::Second), -1)] },
    MegabitPerSecond { short: "Mbps", long: "megabit per second", aka: [], scalar: ratio!(1 / 1),
        base: [(Quantity::Data(Data::Megabit), 1), (Quantity::Time(Time::Second), -1)] },
    GigabitPerSecond { short: "Gbps", long: "gigabit per second", aka: [], scalar: ratio!(1 / 1),
        base: [(Quantity::Data(Data::Gigabit), 1), (Quantity::Time(Time::Second), -1)] },

    Coulomb { short: "C", long: "coulomb", aka: ["coulombs"], scalar: ratio!(1 / 1),
        base: [(Quantity::Current(ElectricCurrent::Ampere), 1), (Quantity::Time(Time::Second), 1)] },
    Volt { short: "V", long: "volt", aka: ["volts"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -3), (Quantity::Current(ElectricCurrent::Ampere), -1)] },
    Ohm { short: "ohm", long: "ohm", aka: ["ohms", "Ohm", "Ohms"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -3), (Quantity::Current(ElectricCurrent::Ampere), -2)] },
    Siemens { short: "S", long: "siemens", aka: ["siemenses"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), -1), (Quantity::Length(Length::Meter), -2), (Quantity::Time(Time::Second), 3), (Quantity::Current(ElectricCurrent::Ampere), 2)] },
    Farad { short: "F", long: "farad", aka: ["farads"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), -1), (Quantity::Length(Length::Meter), -2), (Quantity::Time(Time::Second), 4), (Quantity::Current(ElectricCurrent::Ampere), 2)] },
    Henry { short: "H", long: "henry", aka: ["henries", "henrys"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2), (Quantity::Current(ElectricCurrent::Ampere), -2)] },
    Weber { short: "Wb", long: "weber", aka: ["webers"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Length(Length::Meter), 2), (Quantity::Time(Time::Second), -2), (Quantity::Current(ElectricCurrent::Ampere), -1)] },
    Tesla { short: "T", long: "tesla", aka: ["teslas"], scalar: ratio!(1 / 1),
        base: [(Quantity::Mass(Mass::Kilogram, None), 1), (Quantity::Time(Time::Second), -2), (Quantity::Current(ElectricCurrent::Ampere), -1)] },

    Percent { short: "%", long: "percent", aka: ["pct"], scalar: ratio!(1 / 100), base: [] },
    Permille { short: "permille", long: "permille", aka: [], scalar: ratio!(1 / 1_000), base: [] },
    Ppm { short: "ppm", long: "parts per million", aka: [], scalar: ratio!(1 / 1_000_000), base: [] },
    Ppb { short: "ppb", long: "parts per billion", aka: [], scalar: ratio!(1 / 1_000_000_000), base: [] },
    Ppt { short: "ppt", long: "parts per trillion", aka: [], scalar: ratio!(1 / 1_000_000_000_000), base: [] },
    BasisPoint { short: "bp", long: "basis point", aka: ["bips", "basispoint", "basispoints"], scalar: ratio!(1 / 10_000), base: [] },
}

// Prefixes apply to coherent SI-derived units and to the conventional forms
// `bar`, `L`, `are`, `Wh`, and `eV`. They do not apply to imperial, speed, or
// scalar units where the notation would be misleading.
si_prefixable!(
    DerivedUnit,
    [
        Newton,
        Joule,
        Watt,
        Pascal,
        Hertz,
        Coulomb,
        Volt,
        Ohm,
        Siemens,
        Farad,
        Henry,
        Weber,
        Tesla,
        Liter,
        Are,
        Bar,
        WattHour,
        Electronvolt,
    ]
);

impl DerivedUnit {
    /// The physical dimensionality (base-axis exponents) of this unit.
    pub fn dimensionality(&self) -> Dimensionality {
        let mut dim = Dimensionality::default();
        for (quantity, power) in self.expansion() {
            dim.add_axis(quantity.base_dimension(), Float::from(power));
        }
        dim
    }

    /// The conversion factor to canonical base units (g, m, s, mol, rad, K, A,
    /// bit): one of this unit equals `factor()` canonical units.
    pub fn factor(&self) -> Float {
        let mut factor = self.scalar();
        for (quantity, power) in self.expansion() {
            factor = &factor * &quantity.get_ratio().pow(&Float::from(power));
        }
        factor
    }

    /// Whether this is the *canonical* named unit for its dimensionality: a
    /// unit whose scalar is exactly 1 (N, J, W, Pa, Hz, V, …), rather than a
    /// scaled or imperial variant (kN, kWh, psi). Only canonical units are
    /// considered as targets for an automatic named collapse, so
    /// `kg*m/s^2` prefers `N` rather than being drowned out by `kN`, `dyn`,
    /// `lbf`, or `kgf`.
    pub fn is_named_canonical(&self) -> bool {
        self.scalar() == Float::from(1)
    }
}

impl FromStr for DerivedUnit {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        DerivedUnit::ALL
            .iter()
            .copied()
            .find(|unit| unit.aliases().contains(&s))
            .ok_or(())
    }
}
