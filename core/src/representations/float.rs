use astro_float::Consts;
use astro_float::RoundingMode;
use astro_float::{BigFloat, Radix};
use std::fmt::Display;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::sync::{Mutex, OnceLock};

fn const_cache() -> &'static Mutex<Consts> {
    static CONST_CACHE: OnceLock<Mutex<Consts>> = OnceLock::new();
    CONST_CACHE
        .get_or_init(|| Mutex::new(Consts::new().expect("Failed to initialize constants")))
}

const PRECISION: usize = 256;
const ROUNDING_MODE: RoundingMode = RoundingMode::ToEven;
const MAX_LEN_BEFORE_SCIENTIFIC_NOTATION: i32 = 9;
pub const INEXACT_SIGNIFICANT_DIGITS: usize = 50;

#[derive(Debug, Clone)]
pub struct Float(BigFloat);

impl Float {
    pub fn parse(s: &str) -> Result<Self, ()> {
        let mut cache = const_cache().lock().unwrap();
        let s = s.trim().replace('_', "").replace(',', "");
        let res = BigFloat::parse(&s, Radix::Dec, PRECISION, ROUNDING_MODE, &mut *cache);
        if res.is_nan() {
            Err(())
        } else {
            Ok(Self(res))
        }
    }

    pub fn pi() -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(cache.pi(PRECISION, ROUNDING_MODE))
    }

    pub fn e() -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(cache.e(PRECISION, ROUNDING_MODE))
    }

    pub fn pow(&self, other: &Self) -> Self {
        // Integer exponents, including negatives, go through astro-float:
        // it handles negative bases and is exact and fast for this case.
        if other.is_integer() {
            let mut cache = const_cache().lock().unwrap();
            return Self(self.0.pow(&other.0, PRECISION, ROUNDING_MODE, &mut *cache));
        }
        // For non-integer exponents, compute `x^y = exp(y * ln x)` at fixed
        // precision. astro-float's `pow` can become pathologically slow when a
        // result is a near-exact integer such as `27^(1/3) = 3`.
        let ln_x = self.ln();
        let product = other * &ln_x;
        product.exp()
    }

    pub fn is_integer(&self) -> bool {
        (self - &self.floor()) == Float::from(0.0)
    }

    pub fn is_nan(&self) -> bool {
        self.0.is_nan()
    }

    pub fn ln(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.ln(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn log10(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.log10(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn sqrt(&self) -> Self {
        Self(self.0.sqrt(PRECISION, ROUNDING_MODE))
    }

    pub fn sin(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.sin(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn cos(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.cos(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn tan(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.tan(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn asin(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.asin(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn acos(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.acos(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn atan(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.atan(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    /// Four-quadrant inverse tangent. Returns `None` when both inputs are zero.
    pub fn atan2(&self, x: &Self) -> Option<Self> {
        let zero = Float::from(0);
        let angle = if x > &zero {
            (self / x).atan()
        } else if x < &zero && self >= &zero {
            &((self / x).atan()) + &Float::pi()
        } else if x < &zero {
            &((self / x).atan()) - &Float::pi()
        } else if self > &zero {
            &Float::pi() / &Float::from(2)
        } else if self < &zero {
            (&Float::pi() / &Float::from(2)).neg()
        } else {
            return None;
        };
        Some(angle)
    }

    pub fn sinh(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.sinh(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn cosh(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.cosh(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn tanh(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.tanh(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn exp(&self) -> Self {
        let mut cache = const_cache().lock().unwrap();
        Self(self.0.exp(PRECISION, ROUNDING_MODE, &mut *cache))
    }

    pub fn recip(&self) -> Self {
        Self(self.0.reciprocal(PRECISION, ROUNDING_MODE))
    }

    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    pub fn floor(&self) -> Self {
        Self(self.0.floor())
    }

    pub fn ceil(&self) -> Self {
        Self(self.0.ceil())
    }

    pub fn trunc(&self) -> Self {
        if self < &Float::from(0) {
            self.ceil()
        } else {
            self.floor()
        }
    }

    /// Rounds to a fixed number of decimal places.
    ///
    /// Astro-float works in binary, so inexact transcendental results can leave
    /// tiny artifacts such as `sin(2pi) = 2E-77`. Scaling by a decimal power
    /// before rounding gives the calculator a predictable decimal boundary.
    pub fn round_decimal_places(&self, places: i32) -> Self {
        let scale = Float::parse(&format!("1E{places}")).expect("valid decimal scale");
        let scaled = self * &scale;
        let half = Float::from(0.5);
        let zero = Float::from(0.0);

        let rounded = match scaled.partial_cmp(&zero) {
            Some(std::cmp::Ordering::Less) => {
                let magnitude = scaled.neg();
                (&magnitude + &half).floor().neg()
            }
            Some(_) => (&scaled + &half).floor(),
            None => return self.clone(),
        };

        if rounded == zero {
            zero
        } else {
            &rounded / &scale
        }
    }

    /// Rounds to a number of significant decimal digits, preserving the scale
    /// of very small and very large values.
    pub fn round_significant_digits(&self, digits: usize) -> Self {
        if digits == 0 || self == &Float::from(0) || self.0.exponent().is_none() {
            return self.clone();
        }

        let ten = Float::from(10);
        let one = Float::from(1);
        let mut normalized = self.abs();
        let mut decimal_exponent = 0i32;
        while normalized >= ten {
            normalized = &normalized / &ten;
            decimal_exponent += 1;
        }
        while normalized < one {
            normalized = &normalized * &ten;
            decimal_exponent -= 1;
        }

        self.round_decimal_places(digits as i32 - 1 - decimal_exponent)
    }

    /// Applies the calculator's standard rounding for inexact functions.
    pub fn round_inexact(&self) -> Self {
        self.round_significant_digits(INEXACT_SIGNIFICANT_DIGITS)
    }

    /// Returns true if this value is a non-negative integer.
    pub fn is_nonneg_integer(&self) -> bool {
        matches!(
            self.partial_cmp(&Float::from(0.0)),
            Some(std::cmp::Ordering::Greater) | Some(std::cmp::Ordering::Equal)
        ) && self.is_integer()
    }

    /// The gamma function, computed with the Lanczos approximation (g = 7).
    /// `factorial(n) == gamma(n + 1)`.
    pub fn gamma(&self) -> Self {
        let half = Float::from(0.5);
        let one = Float::from(1.0);

        // Reflection formula for x < 0.5: Γ(x) = π / (sin(πx) · Γ(1 - x))
        if matches!(self.partial_cmp(&half), Some(std::cmp::Ordering::Less)) {
            let pi = Float::pi();
            let sin_pi_x = (&pi * self).sin();
            let gamma_1_minus = (&one - self).gamma();
            return &pi / &(&sin_pi_x * &gamma_1_minus);
        }

        // Lanczos coefficients (g = 7, n = 9).
        const C: [f64; 9] = [
            0.999_999_999_999_809_93,
            676.520_368_121_885_1,
            -1_259.139_216_722_402_8,
            771.323_428_777_653_13,
            -176.615_029_162_140_59,
            12.507_343_278_686_905,
            -0.138_571_095_265_720_12,
            9.984_369_578_019_571_6e-6,
            1.505_632_735_149_311_6e-7,
        ];

        let z = self - &one;
        let mut x = Float::from(C[0]);
        for (i, &c) in C.iter().enumerate().skip(1) {
            let denom = &z + &Float::from(i as f64);
            x = &x + &(&Float::from(c) / &denom);
        }

        let g = Float::from(7.0);
        let t = &(&z + &g) + &half;
        let sqrt_2pi = (&Float::pi() * &Float::from(2.0)).sqrt();
        let t_pow = t.pow(&(&z + &half));
        let exp_neg_t = t.clone().neg().exp();

        &(&(&sqrt_2pi * &t_pow) * &exp_neg_t) * &x
    }
}

impl Display for Float {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut cache = const_cache().lock().unwrap();
        let string = self
            .0
            .format(Radix::Dec, ROUNDING_MODE, &mut *cache)
            .unwrap();

        let string = string.split('e').collect::<Vec<&str>>();
        let (Some(mantissa), Some(exponent)) = (string.get(0), string.get(1)) else {
            write!(f, "{}", string[0])?;
            return Ok(());
        };

        let mut exponent = exponent.parse::<i32>().unwrap();

        let mut mantissa = mantissa.to_string();

        let mut sign = false;

        if mantissa.starts_with('-') {
            mantissa.remove(0);
            sign = true;
        }

        mantissa.remove(1);

        mantissa.truncate(30);
        while mantissa.len() < 30 {
            mantissa.push('0');
        }

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

        // Rounding a mantissa made entirely of nines can collapse it to only
        // one or two digits. Restore the working width before inserting the
        // decimal point below.
        while mantissa.len() < 30 {
            mantissa.push('0');
        }

        if exponent <= MAX_LEN_BEFORE_SCIENTIFIC_NOTATION
            && exponent >= -MAX_LEN_BEFORE_SCIENTIFIC_NOTATION
        {
            mantissa.insert_str(0, &"0".repeat(MAX_LEN_BEFORE_SCIENTIFIC_NOTATION as usize));

            mantissa.insert_str(
                (MAX_LEN_BEFORE_SCIENTIFIC_NOTATION + 1 + exponent) as usize,
                ".",
            );

            while mantissa.ends_with('0') {
                mantissa.pop();
            }

            if mantissa.ends_with('.') {
                mantissa.pop();
            }

            while mantissa.starts_with('0') {
                mantissa.remove(0);
            }

            if mantissa.starts_with('.') {
                mantissa.insert(0, '0');
            }

            let mut i = 0;
            while i < mantissa.len() {
                if mantissa.chars().nth(i).unwrap() == '.' {
                    break;
                }
                i += 1;
            }

            if i > 3 {
                let mut i = i - 3;

                loop {
                    mantissa.insert(i, ',');
                    if i > 3 {
                        i -= 3;
                    } else {
                        break;
                    }
                }
            }

            if sign {
                mantissa.insert(0, '-');
            }

            write!(f, "{}", mantissa)?;
            Ok(())
        } else {
            mantissa.insert(1, '.');

            while mantissa.ends_with('0') {
                mantissa.pop();
            }

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
        assert!(
            (&Float::parse("1.0E-1").unwrap() - &Float::from(0.1)).abs()
                < Float::from(0.0000000001)
        );
        assert!(
            (&Float::parse("15.03E+3").unwrap() - &Float::from(15030)).abs()
                < Float::from(0.0000000001)
        );
    }

    #[test]
    fn test_string() {
        assert_eq!(Float::parse("-10.123").unwrap().to_string(), "-10.123");
        assert_eq!(
            Float::parse("-1,000,000.245").unwrap().to_string(),
            "-1,000,000.245"
        );
        assert_eq!(Float::parse("001.00").unwrap().to_string(), "1");
        assert_eq!(Float::parse("5e-20").unwrap().to_string(), "5E-20");
        assert_eq!(Float::parse("999").unwrap().to_string(), "999");
        assert_eq!(
            Float::parse(".01123410918273418734182374")
                .unwrap()
                .to_string(),
            "0.01123410918273418734182374"
        );
        assert_eq!(Float::parse("0").unwrap().to_string(), "0.0");
        assert_eq!(Float::from(0).to_string(), "0.0");
        assert_eq!(Float::from(-1).to_string(), "-1");
        assert_eq!(Float::from(f64::NAN).to_string(), "NaN");
        assert_eq!(Float::from(f64::INFINITY).to_string(), "Inf");
        assert_eq!(Float::from(f64::NEG_INFINITY).to_string(), "-Inf");
    }

    #[test]
    fn rounds_to_fixed_decimal_places() {
        assert_eq!(
            Float::parse("1.23456").unwrap().round_decimal_places(4),
            Float::parse("1.2346").unwrap()
        );
        assert_eq!(
            Float::parse("-1.23456").unwrap().round_decimal_places(4),
            Float::parse("-1.2346").unwrap()
        );
        assert_eq!(
            Float::parse("1.23456E-77")
                .unwrap()
                .round_significant_digits(4),
            Float::parse("1.235E-77").unwrap()
        );
        assert_eq!(
            Float::parse("1.23456E77")
                .unwrap()
                .round_significant_digits(4),
            Float::parse("1.235E77").unwrap()
        );
    }
}
