#[macro_export]
macro_rules! ratio {
    ($num:literal / $denom:literal) => {{
        use rug::Rational;

        let mut num =
            Rational::from_str(concat!(stringify!($num), "/", stringify!($denom))).unwrap();

        num
    }};
}

/// Macro that generates a new enum for a quantity.
/// The first passed string for each unit should be the unit's default shorthand name.
/// The second passed string for each unit should be the unit's default longhand name.
/// Any other passed strings for each unit should be other possible names/abbreviations of the unit.
/// ```rust
/// use siffra_desktop::{quantity, ratio};
/// quantity!(Length, [(Meter, ratio!(1 / 1), "m", "meter", "meters")]);
/// ```
#[macro_export]
macro_rules! quantity {
    ($name:ident, [$(($unit_name:ident, $unit_ratio:expr, $default_shorthand:expr, $default_longhand:expr, $($unit_aliases:expr),*)),*]) => {
        use rug::Rational;
        use std::str::FromStr;

        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum $name {
            $($unit_name),*
        }

        impl FromStr for $name {
            type Err = ();

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($default_shorthand | $default_longhand | $($unit_aliases)|* => Ok($name::$unit_name)),*,
                    _ => Err(())
                }
            }
        }

        impl $name {
            pub fn ratio(&self) -> Rational {
                match self {
                    $($name::$unit_name => $unit_ratio),*
                }
            }

            pub fn shorthand(&self) -> &'static str {
                match self {
                    $($name::$unit_name => $default_shorthand),*
                }
            }

            pub fn longhand(&self) -> &'static str {
                match self {
                    $($name::$unit_name => $default_longhand),*
                }
            }

            pub fn aliases(&self) -> Vec<&'static str> {
                match self {
                    $($name::$unit_name => vec![$default_shorthand, $default_longhand, $($unit_aliases),*]),*
                }
            }
        }
    };
}
