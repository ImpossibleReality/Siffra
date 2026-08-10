//! Macros for declaring builtins.
//!
//! A definition names the function, documents it, and points at the
//! implementation, so everything about one builtin sits in one place and the
//! documentation is written where the code is:
//!
//! ```ignore
//! pub(super) const DEFINITIONS: &[Function] = siffra_functions![
//!     {
//!         name: "hypot",
//!         category: "Arithmetic",
//!         params: (a, b),
//!         description: "Length of the hypotenuse formed by two values with compatible units.",
//!         examples: ["hypot(3 m, 400 cm)" => "5 m"],
//!         eval: hypot,
//!     },
//! ];
//!
//! fn hypot(call: &Call) -> Result<Value, SiffraExecutionError> { … }
//! ```
//!
//! `aliases`, `base` and `notations` may be omitted; when present they appear
//! in that order, between `name`/`category` and `params`/`description`.

/// The argument list of a function, in one of three shapes:
///
/// - `(x, low, high)` — exactly these arguments.
/// - `(x; places)` — required arguments, then ones that may be left off.
/// - `(a, b; min 1)` — any number from `min` upward, spelled `a, b, …`.
macro_rules! siffra_params {
    (($($shown:ident),+ ; min $min:literal)) => {
        $crate::registry::Params::Variadic {
            shown: &[$(stringify!($shown)),+],
            min: $min,
        }
    };
    (($($required:ident),* ; $($optional:ident),+)) => {
        $crate::registry::Params::Optional {
            required: &[$(stringify!($required)),*],
            optional: &[$(stringify!($optional)),+],
        }
    };
    (($($name:ident),* $(,)?)) => {
        $crate::registry::Params::Fixed(&[$(stringify!($name)),*])
    };
}

/// Expands to the declared base, or to [`Base::None`] when none was declared.
macro_rules! siffra_base {
    () => {
        $crate::registry::Base::None
    };
    ($name:literal) => {
        $crate::registry::Base::Optional($name)
    };
}

/// Expands to a possibly-absent list of names, so optional fields can be left
/// out of a definition entirely.
macro_rules! siffra_names {
    () => {
        &[]
    };
    ($($name:literal),+) => {
        &[$($name),+]
    };
}

/// Declares a module's function definitions. See the module documentation.
macro_rules! siffra_functions {
    ($({
        name: $name:literal,
        $(aliases: [$($alias:literal),+ $(,)?],)?
        category: $category:literal,
        $(base: $base:literal,)?
        $(notations: [$($notation:literal),+ $(,)?],)?
        params: $params:tt,
        description: $description:expr,
        examples: [$($input:literal => $result:literal),* $(,)?],
        eval: $eval:expr,
    }),* $(,)?) => {
        &[$($crate::registry::Function {
            name: $name,
            aliases: $crate::registry::siffra_names!($($($alias),+)?),
            category: $category,
            params: $crate::registry::siffra_params!($params),
            base: $crate::registry::siffra_base!($($base)?),
            notations: $crate::registry::siffra_names!($($($notation),+)?),
            description: $description,
            examples: &[$($crate::registry::Example {
                input: $input,
                result: $result,
            }),*],
            eval: $eval,
        }),*]
    };
}

/// Declares a module's constant definitions. `value` is a function of no
/// arguments, because the numeric representation cannot be built at compile
/// time.
macro_rules! siffra_constants {
    ($({
        name: $name:literal,
        $(aliases: [$($alias:literal),+ $(,)?],)?
        description: $description:expr,
        examples: [$($input:literal => $result:literal),* $(,)?],
        value: $value:expr,
    }),* $(,)?) => {
        &[$($crate::registry::Constant {
            name: $name,
            aliases: $crate::registry::siffra_names!($($($alias),+)?),
            description: $description,
            examples: &[$($crate::registry::Example {
                input: $input,
                result: $result,
            }),*],
            value: $value,
        }),*]
    };
}

pub(crate) use {siffra_base, siffra_constants, siffra_functions, siffra_names, siffra_params};
