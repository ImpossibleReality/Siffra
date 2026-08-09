//! A global catalog of callable functions, named constants, and units
//! with their aliases.

use serde::Serialize;

use crate::representations::{
    AbsoluteTemperature, Amount, Angle, Currency, Data, DerivedUnit, ElectricCurrent, Length, Mass,
    Prefix, SiPrefixable, Time,
};

#[derive(Serialize)]
pub struct Catalog {
    pub functions: Vec<CatalogFunction>,
    pub constants: Vec<CatalogConstant>,
    pub units: Vec<CatalogUnit>,
    pub prefixes: Vec<CatalogPrefix>,
}

#[derive(Serialize)]
pub struct CatalogPrefix {
    pub symbol: String,
    pub name: String,
    pub exponent: i32,
}

#[derive(Serialize)]
pub struct CatalogExample {
    pub input: String,
    pub result: String,
}

#[derive(Serialize)]
pub struct CatalogFunction {
    pub category: String,
    pub name: String,
    pub signature: String,
    pub description: String,
    pub examples: Vec<CatalogExample>,
}

#[derive(Serialize)]
pub struct CatalogConstant {
    pub name: String,
    pub description: String,
    pub examples: Vec<CatalogExample>,
}

#[derive(Serialize)]
pub struct CatalogUnit {
    pub category: String,
    pub name: String,
    pub symbol: String,
    pub aliases: Vec<String>,
    pub si_prefixes: bool,
}

pub fn catalog() -> Catalog {
    Catalog {
        functions: functions(),
        constants: constants(),
        units: units(),
        prefixes: prefixes(),
    }
}

fn prefixes() -> Vec<CatalogPrefix> {
    Prefix::ALL
        .iter()
        .map(|prefix| CatalogPrefix {
            symbol: prefix.symbol().to_string(),
            name: prefix.name().to_string(),
            exponent: prefix.exponent(),
        })
        .collect()
}

fn ex(input: &str, result: &str) -> CatalogExample {
    CatalogExample {
        input: input.to_string(),
        result: result.to_string(),
    }
}

fn functions() -> Vec<CatalogFunction> {
    let f = |category: &str,
             name: &str,
             signature: &str,
             description: &str,
             examples: Vec<CatalogExample>| CatalogFunction {
        category: category.to_string(),
        name: name.to_string(),
        signature: signature.to_string(),
        description: description.to_string(),
        examples,
    };
    vec![
        f(
            "Powers & logarithms",
            "sqrt",
            "sqrt(x)",
            "Square root of x.",
            vec![ex("sqrt(144)", "12")],
        ),
        f(
            "Powers & logarithms",
            "root",
            "root(x)  ·  root_n(x)",
            "The nth root of x. Write the degree after an underscore; one argument gives the square root.",
            vec![ex("root(144)", "12"), ex("root_3(27)", "3")],
        ),
        f(
            "Powers & logarithms",
            "log",
            "log(x)  ·  log_b(x)",
            "Base-10 logarithm, or base-b when you write the base after an underscore.",
            vec![ex("log(1000)", "3"), ex("log_2(32)", "5")],
        ),
        f(
            "Powers & logarithms",
            "ln",
            "ln(x)",
            "Natural logarithm (base e).",
            vec![ex("ln(e)", "1")],
        ),
        f(
            "Powers & logarithms",
            "exp",
            "exp(x)",
            "e raised to the power of x.",
            vec![ex("exp(1)", "2.718282")],
        ),
        f(
            "Rounding",
            "round",
            "round(x)  ·  round(x, places)",
            "Rounds x to the nearest integer or to the requested decimal places; keeps the unit of x.",
            vec![
                ex("round(12.345, 2)", "12.35"),
                ex("round(1250, -2)", "1,300"),
            ],
        ),
        f(
            "Rounding",
            "floor",
            "floor(x)",
            "Rounds x down to the nearest integer; keeps its unit.",
            vec![ex("floor(-1.2 kg)", "-2 kg")],
        ),
        f(
            "Rounding",
            "ceil",
            "ceil(x)",
            "Rounds x up to the nearest integer; keeps its unit.",
            vec![ex("ceil(-1.2 kg)", "-1 kg")],
        ),
        f(
            "Rounding",
            "trunc",
            "trunc(x)",
            "Removes the fractional part of x by rounding toward zero; keeps its unit.",
            vec![ex("trunc(-1.9 kg)", "-1 kg")],
        ),
        f(
            "Arithmetic",
            "abs",
            "abs(x)",
            "Absolute value; keeps the unit of x.",
            vec![ex("abs(-42)", "42")],
        ),
        f(
            "Arithmetic",
            "sign",
            "sign(x)",
            "Returns -1, 0, or 1 according to the sign of x.",
            vec![ex("sign(-42 m)", "-1")],
        ),
        f(
            "Arithmetic",
            "mod",
            "mod(a, b)",
            "Euclidean remainder after division; a and b must have compatible units.",
            vec![ex("mod(5 m, 200 cm)", "1 m")],
        ),
        f(
            "Arithmetic",
            "hypot",
            "hypot(a, b)",
            "Length of the hypotenuse formed by two values with compatible units.",
            vec![ex("hypot(3 m, 400 cm)", "5 m")],
        ),
        f(
            "Aggregation",
            "min",
            "min(a, b, …)",
            "The smallest of one or more values with compatible units.",
            vec![ex("min(2 m, 150 cm, 3 m)", "1.5 m")],
        ),
        f(
            "Aggregation",
            "max",
            "max(a, b, …)",
            "The largest of one or more values with compatible units.",
            vec![ex("max(2 m, 150 cm, 3 m)", "3 m")],
        ),
        f(
            "Aggregation",
            "mean",
            "mean(a, b, …)",
            "Arithmetic mean of one or more values with compatible units.",
            vec![ex("mean(1 m, 200 cm, 3 m)", "2 m")],
        ),
        f(
            "Aggregation",
            "clamp",
            "clamp(x, low, high)",
            "Limits x to an inclusive range whose bounds have compatible units.",
            vec![ex("clamp(250 cm, 1 m, 2 m)", "200 cm")],
        ),
        f(
            "Combinatorics",
            "gcd",
            "gcd(a, b)",
            "Greatest common divisor of two unitless integers.",
            vec![ex("gcd(84, 30)", "6")],
        ),
        f(
            "Combinatorics",
            "lcm",
            "lcm(a, b)",
            "Least common multiple of two unitless integers.",
            vec![ex("lcm(84, 30)", "420")],
        ),
        f(
            "Combinatorics",
            "choose",
            "choose(n, k)",
            "Number of ways to choose k items from n without regard to order.",
            vec![ex("choose(10, 3)", "120")],
        ),
        f(
            "Combinatorics",
            "permute",
            "permute(n, k)",
            "Number of ordered selections of k items from n.",
            vec![ex("permute(10, 3)", "720")],
        ),
        f(
            "Combinatorics",
            "factorial",
            "factorial(x)  ·  x!",
            "Factorial of x, extended to non-integers via the gamma function.",
            vec![ex("factorial(6)", "720")],
        ),
        f(
            "Combinatorics",
            "gamma",
            "gamma(x)",
            "Gamma function Γ(x), with Γ(n) = (n - 1)! for positive integers.",
            vec![ex("gamma(0.5)", "1.772454")],
        ),
        f(
            "Finance",
            "apy",
            "apy(apr, compounds)  ·  ear(apr, compounds)",
            "Annual percentage yield (effective annual rate) from an APR and compounding periods per year. Accepts a percentage or decimal APR.",
            vec![ex("apy(5%, 12)", "5.11619 %")],
        ),
        f(
            "Finance",
            "apr",
            "apr(apy, compounds)",
            "Nominal annual percentage rate corresponding to an APY and compounding frequency.",
            vec![ex("apr(5.11619%, 12)", "5 %")],
        ),
        f(
            "Finance",
            "fv",
            "fv(principal, rate, periods)",
            "Future value after compounding a principal at a per-period rate.",
            vec![ex("fv(1000 USD, 5%, 10)", "1,628.894627 USD")],
        ),
        f(
            "Finance",
            "pv",
            "pv(future, rate, periods)",
            "Present value of a future amount discounted at a per-period rate.",
            vec![ex("pv(1628.894627 USD, 5%, 10)", "1,000 USD")],
        ),
        f(
            "Finance",
            "pmt",
            "pmt(principal, rate, periods)",
            "Equal end-of-period payment needed to amortize a principal.",
            vec![ex("pmt(1000 USD, 5%, 12)", "112.82541 USD")],
        ),
        f(
            "Finance",
            "compound",
            "compound(principal, rate, periods)",
            "Compounds a principal at a per-period rate; an explanatory alias for fv.",
            vec![ex("compound(1000 USD, 5%, 10)", "1,628.894627 USD")],
        ),
        f(
            "Trigonometry",
            "sin",
            "sin(x)",
            "Sine of x (x in radians).",
            vec![ex("sin(pi / 2)", "1")],
        ),
        f(
            "Trigonometry",
            "cos",
            "cos(x)",
            "Cosine of x (x in radians).",
            vec![ex("cos(0)", "1")],
        ),
        f(
            "Trigonometry",
            "tan",
            "tan(x)",
            "Tangent of x (x in radians).",
            vec![ex("tan(pi / 4)", "1")],
        ),
        f(
            "Trigonometry",
            "sinh",
            "sinh(x)",
            "Hyperbolic sine of a unitless x.",
            vec![ex("sinh(1)", "1.175201")],
        ),
        f(
            "Trigonometry",
            "cosh",
            "cosh(x)",
            "Hyperbolic cosine of a unitless x.",
            vec![ex("cosh(1)", "1.543081")],
        ),
        f(
            "Trigonometry",
            "tanh",
            "tanh(x)",
            "Hyperbolic tangent of a unitless x.",
            vec![ex("tanh(1)", "0.761594")],
        ),
        f(
            "Trigonometry",
            "asin",
            "asin(x)",
            "Inverse sine, returning radians.",
            vec![ex("asin(1)", "1.570796")],
        ),
        f(
            "Trigonometry",
            "acos",
            "acos(x)",
            "Inverse cosine, returning radians.",
            vec![ex("acos(0)", "1.570796")],
        ),
        f(
            "Trigonometry",
            "atan",
            "atan(x)",
            "Inverse tangent, returning radians.",
            vec![ex("atan(1)", "0.785398")],
        ),
        f(
            "Trigonometry",
            "atan2",
            "atan2(y, x)",
            "Four-quadrant inverse tangent of y and x, returning radians.",
            vec![ex("atan2(1, 1)", "0.785398")],
        ),
        f(
            "Units",
            "simplify",
            "simplify(x)",
            "Collapses a compound unit into the named derived unit that matches it, when exactly one does (kg*m/s^2 -> N, J/s -> W). Leaves x unchanged when no single named unit fits.",
            vec![ex("simplify(1000 kg*m/s^2)", "1,000 N")],
        ),
        f(
            "Units",
            "expand",
            "expand(x)",
            "Expands named derived units into the base units they are built from (N -> kg*m/s^2). The magnitude adjusts so the quantity is unchanged.",
            vec![ex("expand(5 N)", "5 kg*m/s^2")],
        ),
    ]
}

fn constants() -> Vec<CatalogConstant> {
    let c = |name: &str, description: &str, examples: Vec<CatalogExample>| CatalogConstant {
        name: name.to_string(),
        description: description.to_string(),
        examples,
    };
    vec![
        c(
            "pi",
            "The ratio of a circle's circumference to its diameter.",
            vec![ex("pi", "3.141593")],
        ),
        c(
            "e",
            "Euler's number, the base of the natural logarithm.",
            vec![ex("e", "2.718282")],
        ),
        c(
            "tau",
            "τ = 2π, a full turn in radians.",
            vec![ex("tau", "6.283185")],
        ),
        c(
            "phi",
            "The golden ratio, (1 + √5) / 2.",
            vec![ex("phi", "1.618034")],
        ),
        c(
            "lightspeed",
            "The speed of light in vacuum, exactly 299,792,458 m/s.",
            vec![ex("lightspeed", "299,792,458 m/s")],
        ),
        c(
            "standardgravity",
            "Standard acceleration due to gravity, exactly 9.80665 m/s².",
            vec![ex("standardgravity", "9.80665 m/s^2")],
        ),
        c(
            "avogadro",
            "The Avogadro constant, exactly 6.02214076 × 10²³ per mole.",
            vec![ex("avogadro", "6.02214076E23 /mol")],
        ),
        c(
            "planck",
            "The Planck constant, exactly 6.62607015 × 10⁻³⁴ joule-seconds.",
            vec![ex("planck", "6.62607015E-34 J*s")],
        ),
        c(
            "boltzmann",
            "The Boltzmann constant, exactly 1.380649 × 10⁻²³ joules per kelvin.",
            vec![ex("boltzmann", "1.380649E-23 J/iK")],
        ),
        c(
            "elementarycharge",
            "The elementary charge, exactly 1.602176634 × 10⁻¹⁹ coulombs.",
            vec![ex("elementarycharge", "1.602176634E-19 C")],
        ),
        c(
            "k",
            "Shorthand for one thousand (also uppercase K).",
            vec![ex("5k", "5,000")],
        ),
    ]
}

fn units() -> Vec<CatalogUnit> {
    let mut units = Vec::new();

    {
        let symbol = "#";
        let name = "number";
        let aliases: Vec<&str> = crate::parser::SCALAR_UNIT_ALIASES
            .iter()
            .copied()
            .filter(|a| *a != symbol && *a != name)
            .collect();
        units.push(make_unit("Number", symbol, name, &aliases, false));
    }

    for u in Length::ALL {
        units.push(make_unit(
            "Length",
            u.shorthand(),
            u.longhand(),
            &u.aliases(),
            takes_si_prefixes(u),
        ));
    }
    for u in Mass::ALL {
        units.push(make_unit(
            "Mass",
            u.shorthand(),
            u.longhand(),
            &u.aliases(),
            takes_si_prefixes(u),
        ));
    }
    for u in Time::ALL {
        units.push(make_unit(
            "Time",
            u.shorthand(),
            u.longhand(),
            &u.aliases(),
            takes_si_prefixes(u),
        ));
    }
    for u in Angle::ALL {
        units.push(make_unit(
            "Angle",
            u.shorthand(),
            u.longhand(),
            &u.aliases(),
            takes_si_prefixes(u),
        ));
    }
    for u in Data::ALL {
        units.push(make_unit(
            "Data",
            u.shorthand(),
            u.longhand(),
            &u.aliases(),
            false,
        ));
    }
    for u in Amount::ALL {
        units.push(make_unit(
            "Amount of substance",
            u.shorthand(),
            u.longhand(),
            &u.aliases(),
            takes_si_prefixes(u),
        ));
    }
    for u in ElectricCurrent::ALL {
        units.push(make_unit(
            "Electric current",
            u.shorthand(),
            u.longhand(),
            &u.aliases(),
            takes_si_prefixes(u),
        ));
    }

    for u in AbsoluteTemperature::ALL {
        units.push(make_unit(
            "Temperature",
            u.shorthand(),
            u.longhand(),
            u.aliases(),
            false,
        ));
    }

    for u in DerivedUnit::ALL {
        units.push(make_unit(
            derived_category(u),
            u.shorthand(),
            u.longhand(),
            &u.aliases(),
            takes_si_prefixes(u),
        ));
    }

    for currency in Currency::all() {
        units.push(make_unit(
            "Currency",
            currency.code(),
            currency.name(),
            &[],
            false,
        ));
    }

    units
}

fn takes_si_prefixes<T: SiPrefixable + PartialEq>(unit: &T) -> bool {
    T::PREFIXABLE.contains(unit)
}

fn make_unit(
    category: &str,
    symbol: &str,
    name: &str,
    aliases: &[&str],
    si_prefixes: bool,
) -> CatalogUnit {
    let mut extra: Vec<String> = Vec::new();
    for alias in aliases {
        if *alias != symbol && *alias != name && !extra.iter().any(|e| e == alias) {
            extra.push(alias.to_string());
        }
    }
    CatalogUnit {
        category: category.to_string(),
        name: name.to_string(),
        symbol: symbol.to_string(),
        aliases: extra,
        si_prefixes,
    }
}

fn derived_category(unit: &DerivedUnit) -> &'static str {
    use DerivedUnit::*;
    match unit {
        Newton | Dyne | PoundForce | KilogramForce => "Force",
        Joule | WattHour | Calorie | Kilocalorie | Btu | Electronvolt | Erg | Therm | FootPound => {
            "Energy"
        }
        Watt | Horsepower | MetricHorsepower => "Power",
        Pascal | Bar | Atmosphere | Psi | Torr | MillimeterMercury | InchMercury => "Pressure",
        Hertz | Rpm => "Frequency",
        MilePerHour | KilometerPerHour | FootPerSecond | Knot | Mach => "Speed",
        Hectare | Acre | Are => "Area",
        Liter | Gallon | Quart | Pint | Cup | FluidOunce | Tablespoon | Teaspoon | Barrel => {
            "Volume"
        }
        BitPerSecond | KilobitPerSecond | MegabitPerSecond | GigabitPerSecond => "Data rate",
        Coulomb | Volt | Ohm | Siemens | Farad | Henry | Weber | Tesla => "Electricity & magnetism",
        Percent | Permille | Ppm | Ppb | Ppt | BasisPoint => "Ratios & percentages",
    }
}
