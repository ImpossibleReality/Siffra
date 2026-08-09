use crate::representations::Float;
use crate::representations::dimension::{BaseDimension, Dimensionality, TemperatureInterval};

/// Affine temperature scale; arithmetic decays it to a linear interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsoluteTemperature {
    Kelvin,
    Celsius,
    Fahrenheit,
    Rankine,
}

impl AbsoluteTemperature {
    /// Multiplicative part of the affine map to Kelvin
    fn scale(&self) -> Float {
        match self {
            AbsoluteTemperature::Kelvin | AbsoluteTemperature::Celsius => Float::from(1),
            AbsoluteTemperature::Fahrenheit | AbsoluteTemperature::Rankine => {
                &Float::from(5) / &Float::from(9)
            }
        }
    }

    fn offset(&self) -> Float {
        match self {
            AbsoluteTemperature::Kelvin | AbsoluteTemperature::Rankine => Float::from(0),
            AbsoluteTemperature::Celsius => Float::parse("273.15").unwrap(),
            AbsoluteTemperature::Fahrenheit => {
                &Float::parse("273.15").unwrap() - &(&self.scale() * &Float::from(32))
            }
        }
    }

    pub fn to_kelvin(&self, value: &Float) -> Float {
        &(&self.scale() * value) + &self.offset()
    }

    pub fn from_kelvin(&self, kelvin: &Float) -> Float {
        &(kelvin - &self.offset()) / &self.scale()
    }

    pub fn interval(&self) -> TemperatureInterval {
        match self {
            AbsoluteTemperature::Kelvin | AbsoluteTemperature::Celsius => {
                TemperatureInterval::Kelvin
            }
            AbsoluteTemperature::Fahrenheit => TemperatureInterval::Fahrenheit,
            AbsoluteTemperature::Rankine => TemperatureInterval::Rankine,
        }
    }

    pub fn dimensionality(&self) -> Dimensionality {
        let mut dim = Dimensionality::default();
        dim.add_axis(BaseDimension::Temperature, Float::from(1));
        dim
    }

    pub const ALL: &'static [AbsoluteTemperature] = &[
        AbsoluteTemperature::Celsius,
        AbsoluteTemperature::Fahrenheit,
        AbsoluteTemperature::Kelvin,
        AbsoluteTemperature::Rankine,
    ];

    pub fn shorthand(&self) -> &'static str {
        match self {
            AbsoluteTemperature::Kelvin => "K",
            AbsoluteTemperature::Celsius => "°C",
            AbsoluteTemperature::Fahrenheit => "°F",
            AbsoluteTemperature::Rankine => "°R",
        }
    }

    pub fn longhand(&self) -> &'static str {
        match self {
            AbsoluteTemperature::Kelvin => "kelvin",
            AbsoluteTemperature::Celsius => "celsius",
            AbsoluteTemperature::Fahrenheit => "fahrenheit",
            AbsoluteTemperature::Rankine => "rankine",
        }
    }

    pub fn aliases(&self) -> &'static [&'static str] {
        match self {
            AbsoluteTemperature::Kelvin => &["K", "kelvin", "kelvins"],
            AbsoluteTemperature::Celsius => &["C", "c", "°C", "celsius", "degC", "centigrade"],
            AbsoluteTemperature::Fahrenheit => &["F", "f", "°F", "fahrenheit", "degF"],
            AbsoluteTemperature::Rankine => &["R", "°R", "rankine", "degR"],
        }
    }

    pub fn from_str(s: &str) -> Result<Self, ()> {
        for unit in [
            AbsoluteTemperature::Kelvin,
            AbsoluteTemperature::Celsius,
            AbsoluteTemperature::Fahrenheit,
            AbsoluteTemperature::Rankine,
        ] {
            if unit.aliases().contains(&s) {
                return Ok(unit);
            }
        }
        Err(())
    }
}
