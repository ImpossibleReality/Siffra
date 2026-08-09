use super::SiffraState;
use super::helpers::{physical_constant, unit_label};
use super::state::VariableAccessError;
use crate::error::SiffraExecutionError;
use crate::parser::Span;
use crate::representations::{
    Amount, DerivedUnit, Float, Length, Quantity, TemperatureInterval, Time, Unit, Value,
};
use crate::siffra_error;
use std::ops::Mul;

pub(super) fn resolve_variable(
    name: &str,
    state: &SiffraState,
    span: Option<Span>,
) -> Result<Value, SiffraExecutionError> {
    if let Ok(value) = state.get_variable(name) {
        Ok(value.clone())
    } else if let Err(VariableAccessError::Error(source)) = state.get_variable(name) {
        let cause = source.description().unwrap_or_else(|| source.message());
        Err(siffra_error!(
            "Variable Error",
            format!("Variable '{name}' is invalid: {cause}"),
            span
        ))
    } else {
        match name {
            "pi" => Ok(Value::new(Float::pi(), None)),
            "e" => Ok(Value::new(Float::e(), None)),
            "tau" => Ok(Value::new(
                Float::pi().mul(&Float::parse("2").unwrap()),
                None,
            )),
            "phi" => Ok(Value::new(
                &(&Float::from(1) + &Float::from(5).sqrt()) / &Float::from(2),
                None,
            )),
            "lightspeed" => Ok(physical_constant(
                "299792458",
                vec![
                    (Unit::Base(Quantity::Length(Length::Meter)), 1),
                    (Unit::Base(Quantity::Time(Time::Second)), -1),
                ],
            )),
            "standardgravity" => Ok(physical_constant(
                "9.80665",
                vec![
                    (Unit::Base(Quantity::Length(Length::Meter)), 1),
                    (Unit::Base(Quantity::Time(Time::Second)), -2),
                ],
            )),
            "avogadro" => Ok(physical_constant(
                "6.02214076E23",
                vec![(Unit::Base(Quantity::Amount(Amount::Mole, None)), -1)],
            )),
            "planck" => Ok(physical_constant(
                "6.62607015E-34",
                vec![
                    (Unit::Derived(DerivedUnit::Joule), 1),
                    (Unit::Base(Quantity::Time(Time::Second)), 1),
                ],
            )),
            "boltzmann" => Ok(physical_constant(
                "1.380649E-23",
                vec![
                    (Unit::Derived(DerivedUnit::Joule), 1),
                    (
                        Unit::Base(Quantity::Temperature(TemperatureInterval::Kelvin)),
                        -1,
                    ),
                ],
            )),
            "elementarycharge" => Ok(physical_constant(
                "1.602176634E-19",
                vec![(Unit::Derived(DerivedUnit::Coulomb), 1)],
            )),
            "prev" | "ans" | "previous" | "that" => {
                if let Some(prev) = state.previous_value() {
                    Ok(prev.clone())
                } else {
                    Err(siffra_error!(
                        "Name Error",
                        "No previous value to use",
                        span
                    ))
                }
            }
            "total" | "sum" => {
                if let Some(total) = state.block_total() {
                    Ok(total.clone())
                } else {
                    let description = state.block_total_error().map_or_else(
                        || "Block total is unavailable".to_string(),
                        |(total, value)| {
                            format!(
                                "Block total cannot mix {} and {}",
                                unit_label(total),
                                unit_label(value)
                            )
                        },
                    );
                    Err(siffra_error!("Total Error", description, span))
                }
            }
            "k" | "K" => Ok(Value::new(Float::parse("1000").unwrap(), None)),
            _ => Err(siffra_error!(
                "Name Error",
                format!("Variable '{}' not found", name),
                span
            )),
        }
    }
}
