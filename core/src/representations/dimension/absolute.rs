use crate::representations::Float;
use crate::representations::dimension::{BaseDimension, Dimensionality, TemperatureInterval};

/// An *absolute* (affine) temperature unit. Unlike every other unit in the
/// system — which convert by a pure ratio — these carry an **offset**. The
/// conversion is `kelvin = value * scale + offset`.
///
/// Affine units only make sense as a lone quantity at power +1: `°C²` or
/// `°C·m` have no meaning because the offset does not survive multiplication.
/// So an absolute temperature is a "point" unit; the moment it takes part in
/// any arithmetic it **decays** to its linear interval counterpart
/// ([`AbsoluteTemperature::interval`]), and only
/// [`crate::representations::Value::convert`] applies the affine transform for
/// a standalone power-1 value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsoluteTemperature {
    Kelvin,
    Celsius,
    Fahrenheit,
    Rankine,
}

impl AbsoluteTemperature {
    /// Multiplicative part of the affine map to Kelvin (also the scale of the
    /// matching interval unit).
    fn scale(&self) -> Float {
        match self {
            AbsoluteTemperature::Kelvin | AbsoluteTemperature::Celsius => Float::from(1),
            // 1 °F / 1 °R interval = 5/9 K
            AbsoluteTemperature::Fahrenheit | AbsoluteTemperature::Rankine => {
                &Float::from(5) / &Float::from(9)
            }
        }
    }

    /// Additive part of the affine map to Kelvin: `kelvin = scale*x + offset`.
    fn offset(&self) -> Float {
        match self {
            // Rankine, like Kelvin, is measured from absolute zero.
            AbsoluteTemperature::Kelvin | AbsoluteTemperature::Rankine => Float::from(0),
            AbsoluteTemperature::Celsius => Float::parse("273.15").unwrap(),
            // 273.15 - (5/9)*32
            AbsoluteTemperature::Fahrenheit => {
                &Float::parse("273.15").unwrap() - &(&self.scale() * &Float::from(32))
            }
        }
    }

    /// Converts a magnitude in this unit to absolute Kelvin.
    pub fn to_kelvin(&self, value: &Float) -> Float {
        &(&self.scale() * value) + &self.offset()
    }

    /// Converts an absolute-Kelvin magnitude into this unit.
    pub fn from_kelvin(&self, kelvin: &Float) -> Float {
        &(kelvin - &self.offset()) / &self.scale()
    }

    /// The linear interval unit this decays to under arithmetic (an interval of
    /// 1 °C or 1 K is the same size; 1 °F interval is 5/9 K).
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

    /// Every absolute-temperature unit. Backs the unit catalog (help modal).
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

    /// Human-readable name of this temperature scale.
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
