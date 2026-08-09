#[macro_export]
macro_rules! ratio {
    ($num:literal / $denom:literal) => {{
        use $crate::representations::Float;
        &Float::parse(stringify!($num)).unwrap() / &Float::parse(stringify!($denom)).unwrap()
    }};
}

/// Macro that generates a new enum for a quantity.
/// The first string for each unit is its default shorthand name, the second is
/// its default longhand name, and the rest are aliases or abbreviations.
/// ```ignore
/// use siffra::{quantity, ratio};
/// quantity!(Length, [(Meter, ratio!(1 / 1), "m", "meter", "meters")]);
/// ```
#[macro_export]
macro_rules! quantity {
    ($name:ident, [$(($unit_name:ident, $unit_ratio:expr, $default_shorthand:expr, $default_longhand:expr, $($unit_aliases:expr),*)),*]) => {
        use $crate::representations::Float;
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
            /// Every unit variant of this quantity, in declaration order. Backs
            /// the automatically generated unit catalog (help modal).
            pub const ALL: &'static [$name] = &[$($name::$unit_name),*];

            pub fn ratio(&self) -> Float {
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

/// Declares which members of a unit enum accept SI prefixes.
///
/// List only the units a prefix is meaningful on. Every listed
/// unit then parses with any SI prefix, in both symbol and spelled-out form
/// (`ZF`, `daN`, `nanofarads`), without needing a variant of its own.
///
/// A unit may follow its name with extra aliases that a
/// symbol prefix is allowed to sit on, for spellings that mix the two styles by
/// convention (`msec`, `kgmol`).
///
/// ```ignore
/// si_prefixable!(Length, [Meter]);
/// si_prefixable!(Time, [Second ["sec", "secs"]]);
/// ```
#[macro_export]
macro_rules! si_prefixable {
    ($name:ident, [$($unit_name:ident $([$($extra_symbol:expr),* $(,)?])?),* $(,)?]) => {
        impl $crate::representations::SiPrefixable for $name {
            const PREFIXABLE: &'static [Self] = &[$($name::$unit_name),*];

            fn symbol(&self) -> &'static str {
                $name::shorthand(self)
            }

            fn names(&self) -> Vec<&'static str> {
                $name::aliases(self)
            }

            fn extra_symbol_forms(&self) -> &'static [&'static str] {
                match self {
                    $($name::$unit_name => &[$($($extra_symbol),*)?],)*
                    #[allow(unreachable_patterns)]
                    _ => &[],
                }
            }
        }
    };
}
