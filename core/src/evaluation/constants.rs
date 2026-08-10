//! The named constants, defined alongside their documentation.
//!
//! Values that depend on the state of the document — `prev`, `total` and the
//! like — are not constants and stay with the variable resolution in
//! [`super::variables`].

use crate::registry::{Constant, siffra_constants};
use crate::representations::{
    Amount, DerivedUnit, Dimension, Float, Length, Quantity, TemperatureInterval, Time, Unit, Value,
};
use std::ops::Mul;

pub(crate) const DEFINITIONS: &[Constant] = siffra_constants![
    {
        name: "pi",
        description: "The ratio of a circle's circumference to its diameter.",
        examples: ["pi" => "3.141593"],
        value: || Value::new(Float::pi(), None),
    },
    {
        name: "e",
        description: "Euler's number, the base of the natural logarithm.",
        examples: ["e" => "2.718282"],
        value: || Value::new(Float::e(), None),
    },
    {
        name: "tau",
        description: "τ = 2π, a full turn in radians.",
        examples: ["tau" => "6.283185"],
        value: || Value::new(Float::pi().mul(&Float::parse("2").unwrap()), None),
    },
    {
        name: "phi",
        description: "The golden ratio, (1 + √5) / 2.",
        examples: ["phi" => "1.618034"],
        value: || {
            Value::new(
                &(&Float::from(1) + &Float::from(5).sqrt()) / &Float::from(2),
                None,
            )
        },
    },
    {
        name: "lightspeed",
        description: "The speed of light in vacuum, exactly 299,792,458 m/s.",
        examples: ["lightspeed" => "299,792,458 m/s"],
        value: || {
            physical_constant(
                "299792458",
                &[
                    (Unit::Base(Quantity::Length(Length::Meter)), 1),
                    (Unit::Base(Quantity::Time(Time::Second)), -1),
                ],
            )
        },
    },
    {
        name: "standardgravity",
        description: "Standard acceleration due to gravity, exactly 9.80665 m/s².",
        examples: ["standardgravity" => "9.80665 m/s^2"],
        value: || {
            physical_constant(
                "9.80665",
                &[
                    (Unit::Base(Quantity::Length(Length::Meter)), 1),
                    (Unit::Base(Quantity::Time(Time::Second)), -2),
                ],
            )
        },
    },
    {
        name: "avogadro",
        description: "The Avogadro constant, exactly 6.02214076 × 10²³ per mole.",
        examples: ["avogadro" => "6.02214076E23 1/mol"],
        value: || {
            physical_constant(
                "6.02214076E23",
                &[(Unit::Base(Quantity::Amount(Amount::Mole, None)), -1)],
            )
        },
    },
    {
        name: "planck",
        description: "The Planck constant, exactly 6.62607015 × 10⁻³⁴ joule-seconds.",
        examples: ["planck" => "6.62607015E-34 J*s"],
        value: || {
            physical_constant(
                "6.62607015E-34",
                &[
                    (Unit::Derived(DerivedUnit::Joule), 1),
                    (Unit::Base(Quantity::Time(Time::Second)), 1),
                ],
            )
        },
    },
    {
        name: "boltzmann",
        description: "The Boltzmann constant, exactly 1.380649 × 10⁻²³ joules per kelvin.",
        examples: ["boltzmann" => "1.380649E-23 J/iK"],
        value: || {
            physical_constant(
                "1.380649E-23",
                &[
                    (Unit::Derived(DerivedUnit::Joule), 1),
                    (
                        Unit::Base(Quantity::Temperature(TemperatureInterval::Kelvin)),
                        -1,
                    ),
                ],
            )
        },
    },
    {
        name: "elementarycharge",
        description: "The elementary charge, exactly 1.602176634 × 10⁻¹⁹ coulombs.",
        examples: ["elementarycharge" => "1.602176634E-19 C"],
        value: || physical_constant("1.602176634E-19", &[(Unit::Derived(DerivedUnit::Coulomb), 1)]),
    },
    {
        name: "k",
        aliases: ["K"],
        description: "Shorthand for one thousand (also uppercase K).",
        examples: ["5k" => "5,000"],
        value: || Value::new(Float::parse("1000").unwrap(), None),
    },
];

fn physical_constant(value: &str, terms: &[(Unit, i32)]) -> Value {
    Value::new(
        Float::parse(value).expect("valid physical constant"),
        Some(Dimension::new(
            terms
                .iter()
                .map(|(unit, power)| (unit.clone(), Float::from(*power)))
                .collect(),
        )),
    )
}
