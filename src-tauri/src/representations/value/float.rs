use std::fmt::Display;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::sync::{Arc, Mutex};
use astro_float::{BigFloat, Radix};
use astro_float::Consts;
use astro_float::RoundingMode;
use lazy_static::lazy_static;

lazy_static! {
    static ref CONST_CACHE: Arc<Mutex<Consts>> = Arc::new(Mutex::new(Consts::new().expect("Failed to initialize constants")));
}

const PRECISION: usize = 256;
const ROUNDING_MODE: RoundingMode = RoundingMode::ToEven;

#[derive(Debug, Clone)]
pub struct Float(BigFloat);

impl Float {
    pub fn parse(s: &str) -> Result<Self, ()> {
        let mut cache = CONST_CACHE.lock().unwrap();
        let res = BigFloat::parse(s, Radix::Dec, PRECISION, ROUNDING_MODE, &mut *cache);
        if res.is_nan() {
            Err(())
        } else {
            Ok(Self(res))
        }
    }

    pub fn pi() -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(cache.pi(PRECISION, ROUNDING_MODE))
    }

    pub fn e() -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(cache.e(PRECISION, ROUNDING_MODE))
    }

    pub fn pow(&self, other: &Self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.pow(&other.0, PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn ln(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.ln(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn log10(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.log10(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn sqrt(&self) -> Self {
        Self(self.0.sqrt(PRECISION, ROUNDING_MODE))
    }

    pub fn sin(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.sin(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn cos(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.cos(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn tan(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.tan(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn asin(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.asin(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn acos(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.acos(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn atan(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.atan(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn exp(&self) -> Self {
        let mut cache = CONST_CACHE.lock().unwrap();
        Self(self.0.exp(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn recip(&self) -> Self {
        Self(self.0.reciprocal(PRECISION, ROUNDING_MODE))
    }

    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }
}

impl Display for Float {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut cache = CONST_CACHE.lock().unwrap();
        let string = self.0.format(Radix::Dec, ROUNDING_MODE, &mut *cache).unwrap();

        // In scientific notation
        let string = string.split('e').collect::<Vec<&str>>();
        let (Some(mantissa), Some(exponent)) = (string.get(0), string.get(1)) else {
            write!(f, "{}", string[0])?;
            return Ok(())
        };

        let mut exponent = exponent.parse::<i32>().unwrap();

        // NOTE: mantissa is in format "d.d*"
        let mut mantissa = mantissa.to_string();

        let mut sign = false;

        if mantissa.starts_with('-') {
            mantissa.remove(0);
            sign = true;
        }

        dbg!(mantissa.clone(), exponent.clone(), sign.clone());

        // remove decimal point
        mantissa.remove(1);

        // Truncate to 30 digits
        mantissa.truncate(30);
        // Add zeroes until length is 30
        while mantissa.len() < 30 {
            mantissa.push('0');
        }

        // round trailing nines
        mantissa.insert(0, '0');
        let mut should_round = false;
        while mantissa.ends_with('9') {
            mantissa.pop();
            should_round = true;
        }
        if should_round {
            let mut last_digit = mantissa.pop().unwrap();
            last_digit = (last_digit as u8 + 1) as char;
            mantissa.push(last_digit);
        }
        if !mantissa.starts_with('0') {
            mantissa.insert(0, '0');
            exponent += 2;
        } else {
            mantissa.remove(0);
        }

        if exponent <= 6 && exponent >= -6 {
            // add 6 leading zeros
            mantissa.insert_str(0, "000000");

            // add decimal point
            mantissa.insert_str((7 + exponent) as usize, ".");

            // remove trailing zeros
            while mantissa.ends_with('0') {
                mantissa.pop();
            }

            // remove decimal point if it's the last character
            if mantissa.ends_with('.') {
                mantissa.pop();
            }

            // remove leading zeros
            while mantissa.starts_with('0') {
                mantissa.remove(0);
            }

            if mantissa.starts_with('.') {
                mantissa.insert(0, '0');
            }

            if sign {
                mantissa.insert(0, '-');
            }

            write!(f, "{}", mantissa)?;
            Ok(())
        } else {
            // add decimal point
            mantissa.insert(1, '.');

            while mantissa.ends_with('0') {
                mantissa.pop();
            }

            // remove decimal point if it's the last character
            if mantissa.ends_with('.') {
                mantissa.pop();
            }

            if sign {
                mantissa.insert(0, '-');
            }

            write!(f, "{}E{}", mantissa, exponent)?;

            Ok(())
        }
    }
}

impl Add<&Float> for &Float {
    type Output = Float;

    fn add(self, rhs: &Float) -> Self::Output {
        Float(self.0.add(&rhs.0, PRECISION, ROUNDING_MODE))
    }
}

impl Sub<&Float> for &Float {
    type Output = Float;

    fn sub(self, rhs: &Float) -> Self::Output {
        Float(self.0.sub(&rhs.0, PRECISION, ROUNDING_MODE))
    }
}

impl Mul<&Float> for &Float {
    type Output = Float;

    fn mul(self, rhs: &Float) -> Self::Output {
        Float(self.0.mul(&rhs.0, PRECISION, ROUNDING_MODE))
    }
}

impl Div<&Float> for &Float {
    type Output = Float;

    fn div(self, rhs: &Float) -> Self::Output {
        Float(self.0.div(&rhs.0, PRECISION, ROUNDING_MODE))
    }
}

impl Neg for Float {
    type Output = Float;

    fn neg(self) -> Self::Output {
        Float(self.0.neg())
    }
}

impl PartialEq for Float {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl PartialOrd for Float {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}



impl<T> From<T> for Float
where
    T: Into<BigFloat>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        assert_eq!(Float::parse("1.00").unwrap(), Float::from(1));
        assert_eq!(Float::parse("1.0E1").unwrap(), Float::from(10));
        assert!((&Float::parse("1.0E-1").unwrap() - &Float::from(0.1)).abs() < Float::from(0.0000000001));
        assert!((&Float::parse("15.03E+3").unwrap() - &Float::from(15030)).abs() < Float::from(0.0000000001));
    }

    #[test]
    fn test_string() {
        assert_eq!(Float::parse("-10.123").unwrap().to_string(), "-10.123");
        assert_eq!(Float::parse("001.00").unwrap().to_string(), "1");
        assert_eq!(Float::parse("5e-20").unwrap().to_string(), "5E-20");
        assert_eq!(Float::parse("999").unwrap().to_string(), "999");
        assert_eq!(Float::parse(".01123410918273418734182374").unwrap().to_string(), "0.01123410918273418734182374");
        assert_eq!(Float::parse("0").unwrap().to_string(), "0.0");
        assert_eq!(Float::from(0).to_string(), "0.0");
        assert_eq!(Float::from(-1).to_string(), "-1");
        assert_eq!(Float::from(f64::NAN).to_string(), "NaN");
        assert_eq!(Float::from(f64::INFINITY).to_string(), "Inf");
        assert_eq!(Float::from(f64::NEG_INFINITY).to_string(), "-Inf");
    }
}