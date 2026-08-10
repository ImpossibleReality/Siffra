//! A global catalog of callable functions, named constants, and units
//! with their aliases.
//!
//! Functions and constants are projections of [`crate::registry`], which is
//! where their documentation is written, next to the code that implements
//! them. Only units are gathered here, from the unit definitions themselves.

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

fn functions() -> Vec<CatalogFunction> {
    crate::registry::functions()
        .iter()
        .map(|function| CatalogFunction {
            category: function.category.to_string(),
            name: function.name.to_string(),
            signature: function.signature(),
            description: function.description.to_string(),
            examples: examples(function.examples),
        })
        .collect()
}

fn constants() -> Vec<CatalogConstant> {
    crate::registry::constants()
        .iter()
        .map(|constant| CatalogConstant {
            name: constant.name.to_string(),
            description: constant.description.to_string(),
            examples: examples(constant.examples),
        })
        .collect()
}

fn examples(examples: &'static [crate::registry::Example]) -> Vec<CatalogExample> {
    examples
        .iter()
        .map(|example| CatalogExample {
            input: example.input.to_string(),
            result: example.result.to_string(),
        })
        .collect()
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
