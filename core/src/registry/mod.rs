//! Descriptors for the built-in functions and constants.
//!
//! Every builtin is one value that carries its documentation and its
//! implementation together, so [`crate::catalog`] is derived from the same
//! definitions the evaluator dispatches through instead of being a parallel
//! document that has to be kept in step by hand.
//!
//! Every name a line can call or read resolves through [`function`] and
//! [`constant`] here.

mod macros;

pub(crate) use macros::{
    siffra_base, siffra_constants, siffra_functions, siffra_names, siffra_params,
};

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::error::SiffraExecutionError;
use crate::parser::Span;
use crate::representations::{Expression, Value};

/// The order categories appear in the generated documentation. Functions in a
/// category the list does not name sort after the ones it does, in definition
/// order.
const CATEGORY_ORDER: &[&str] = &[
    "Powers & logarithms",
    "Rounding",
    "Arithmetic",
    "Aggregation",
    "Combinatorics",
    "Finance",
    "Trigonometry",
    "Units",
    "Dates & ranges",
];

/// A worked example. `core/tests/catalog.rs` evaluates every one of these and
/// compares against `result`, so a documented example cannot drift away from
/// what the function actually does.
pub struct Example {
    pub input: &'static str,
    pub result: &'static str,
}

/// How many arguments a function takes, and what the docs call them.
///
/// This covers arity and the names a signature is rendered from. Unit and
/// domain checking stays in the implementation, which is the only place that
/// knows what its own errors should say.
pub enum Params {
    /// Exactly these arguments: `clamp(x, low, high)`.
    Fixed(&'static [&'static str]),
    /// Required arguments followed by ones that may be omitted from the right:
    /// `round(x)` or `round(x, places)`.
    Optional {
        required: &'static [&'static str],
        optional: &'static [&'static str],
    },
    /// Any number of arguments from `min` upward. `shown` only names the ones
    /// the signature spells out before its ellipsis: `min(a, b, …)`.
    Variadic {
        shown: &'static [&'static str],
        min: usize,
    },
}

impl Params {
    /// The accepted argument counts, `None` upper bound meaning unbounded.
    fn bounds(&self) -> (usize, Option<usize>) {
        match self {
            Params::Fixed(names) => (names.len(), Some(names.len())),
            Params::Optional { required, optional } => {
                (required.len(), Some(required.len() + optional.len()))
            }
            Params::Variadic { min, .. } => (*min, None),
        }
    }

    fn accepts(&self, count: usize) -> bool {
        let (min, max) = self.bounds();
        count >= min && max.is_none_or(|max| count <= max)
    }

    /// The argument lists a signature spells out — one per accepted shape.
    fn forms(&self) -> Vec<String> {
        match self {
            Params::Fixed(names) => vec![names.join(", ")],
            Params::Optional { required, optional } => (0..=optional.len())
                .map(|taken| {
                    required
                        .iter()
                        .chain(optional.iter().take(taken))
                        .copied()
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .collect(),
            Params::Variadic { shown, .. } => {
                let mut names: Vec<&str> = shown.to_vec();
                names.push("…");
                vec![names.join(", ")]
            }
        }
    }
}

/// Whether a function accepts the subscript form `f_b(x)`.
///
/// The base arrives as the first argument, so `log_2(8)` and `log(2, 8)` are
/// the same call. Functions that do not opt in reject a base while the line is
/// still being read, rather than reporting a confusing argument count.
pub enum Base {
    None,
    /// Optional, and named as it appears in the signature (`log_b(x)`).
    Optional(&'static str),
}

impl Base {
    fn name(&self) -> Option<&'static str> {
        match self {
            Base::None => None,
            Base::Optional(name) => Some(name),
        }
    }
}

/// One callable builtin: what it is called, what it does, how it is documented,
/// and how to run it.
pub struct Function {
    pub name: &'static str,
    /// Other names for the same function. They resolve to this definition and
    /// share its documentation entry.
    pub aliases: &'static [&'static str],
    pub category: &'static str,
    pub params: Params,
    pub base: Base,
    /// Notations the docs should mention that are not call forms, like `x!`.
    pub notations: &'static [&'static str],
    pub description: &'static str,
    pub examples: &'static [Example],
    pub eval: fn(&Call) -> Result<Value, SiffraExecutionError>,
}

impl Function {
    /// Every name this definition answers to, primary first.
    pub fn names(&self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.name).chain(self.aliases.iter().copied())
    }

    /// The call forms shown in the documentation, e.g. `log(x)  ·  log_b(x)`.
    pub fn signature(&self) -> String {
        let forms = self.params.forms();
        let mut rendered = Vec::new();
        for name in self.names() {
            for args in &forms {
                rendered.push(format!("{name}({args})"));
            }
            if let Some(base) = self.base.name() {
                for args in &forms {
                    rendered.push(format!("{name}_{base}({args})"));
                }
            }
        }
        rendered.extend(self.notations.iter().map(|n| n.to_string()));
        rendered.join("  ·  ")
    }

    /// Whether a call with this many arguments is worth handing to `eval`. A
    /// base counts as an argument, which is what makes `root(x)` and
    /// `root_n(x)` both acceptable.
    fn accepts(&self, count: usize) -> bool {
        if self.params.accepts(count) {
            return true;
        }
        self.base.name().is_some() && count > 0 && self.params.accepts(count - 1)
    }

    /// How to say what this function would have accepted. Written from the
    /// bounds so it stays true when a definition changes.
    fn arity_description(&self, name: &str) -> String {
        let (min, max) = self.params.bounds();
        let max = match (self.base.name(), max) {
            (Some(_), Some(max)) => Some(max + 1),
            (Some(_), None) | (None, None) => None,
            (None, Some(max)) => Some(max),
        };

        let plural = |count: usize| if count == 1 { "argument" } else { "arguments" };
        match max {
            Some(max) if max == min => {
                format!("{name} takes exactly {min} {}", plural(min))
            }
            Some(max) if max == min + 1 => {
                format!("{name} takes {min} or {max} {}", plural(max))
            }
            Some(max) => format!("{name} takes {min} to {max} {}", plural(max)),
            None => format!("{name} takes at least {min} {}", plural(min)),
        }
    }
}

/// One named constant. The value is produced on demand because the numeric
/// representation is not something that can be built at compile time.
pub struct Constant {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub examples: &'static [Example],
    pub value: fn() -> Value,
}

impl Constant {
    pub fn names(&self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.name).chain(self.aliases.iter().copied())
    }
}

/// A resolved call, handed to a function's implementation.
///
/// It carries the name as written so an aliased function can report the name
/// the user typed, and the expression so errors can point at the right span.
pub struct Call<'a> {
    function: &'static Function,
    name: &'a str,
    args: &'a [Value],
    expr: &'a Expression,
}

impl<'a> Call<'a> {
    pub(crate) fn new(
        function: &'static Function,
        name: &'a str,
        args: &'a [Value],
        expr: &'a Expression,
    ) -> Self {
        Self {
            function,
            name,
            args,
            expr,
        }
    }

    pub fn function(&self) -> &'static Function {
        self.function
    }

    /// The name as it was written, which may be an alias.
    pub fn name(&self) -> &str {
        self.name
    }

    pub fn args(&self) -> &'a [Value] {
        self.args
    }

    /// Panics outside the arity the definition declares, which
    /// [`Call::check_arity`] has already enforced before `eval` runs.
    pub fn arg(&self, index: usize) -> &'a Value {
        &self.args[index]
    }

    pub fn arg_count(&self) -> usize {
        self.args.len()
    }

    pub fn expr(&self) -> &'a Expression {
        self.expr
    }

    pub fn span(&self) -> Option<Span> {
        self.expr.span()
    }

    pub(crate) fn check_arity(&self) -> Result<(), SiffraExecutionError> {
        if self.function.accepts(self.args.len()) {
            Ok(())
        } else {
            Err(crate::siffra_error!(
                "Argument Error",
                self.function.arity_description(self.name),
                self.span()
            ))
        }
    }
}

fn all_functions() -> &'static [&'static Function] {
    static FUNCTIONS: OnceLock<Vec<&'static Function>> = OnceLock::new();
    FUNCTIONS.get_or_init(|| {
        let mut functions: Vec<&'static Function> = crate::evaluation::functions::DEFINITIONS
            .iter()
            .flat_map(|items| items.iter())
            .collect();
        let rank = |category: &str| {
            CATEGORY_ORDER
                .iter()
                .position(|known| *known == category)
                .unwrap_or(CATEGORY_ORDER.len())
        };
        // Stable, so definition order still decides within a category.
        functions.sort_by_key(|function| rank(function.category));
        functions
    })
}

fn function_index() -> &'static HashMap<&'static str, &'static Function> {
    static INDEX: OnceLock<HashMap<&'static str, &'static Function>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = HashMap::new();
        for function in all_functions() {
            for name in function.names() {
                if index.insert(name, *function).is_some() {
                    panic!("the function name '{name}' is defined twice");
                }
            }
        }
        index
    })
}

fn constant_index() -> &'static HashMap<&'static str, &'static Constant> {
    static INDEX: OnceLock<HashMap<&'static str, &'static Constant>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index = HashMap::new();
        for constant in constants() {
            for name in constant.names() {
                if index.insert(name, constant).is_some() {
                    panic!("the constant name '{name}' is defined twice");
                }
            }
        }
        index
    })
}

/// Every function, in documentation order.
pub fn functions() -> &'static [&'static Function] {
    all_functions()
}

/// Every constant, in definition order.
pub fn constants() -> &'static [Constant] {
    crate::evaluation::constants::DEFINITIONS
}

/// The function a name denotes, following aliases.
pub fn function(name: &str) -> Option<&'static Function> {
    function_index().get(name).copied()
}

/// The constant a name denotes, following aliases.
pub fn constant(name: &str) -> Option<&'static Constant> {
    constant_index().get(name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_are_built_without_collisions() {
        assert!(function("sqrt").is_some());
        assert!(function("ear").is_some(), "aliases resolve");
        assert_eq!(function("ear").unwrap().name, "apy");
        assert!(function("nope").is_none());
        assert!(constant("pi").is_some());
        assert_eq!(constant("K").unwrap().name, "k");
    }

    #[test]
    fn signatures_render_every_call_form() {
        assert_eq!(function("sqrt").unwrap().signature(), "sqrt(x)");
        assert_eq!(function("log").unwrap().signature(), "log(x)  ·  log_b(x)");
        assert_eq!(
            function("round").unwrap().signature(),
            "round(x)  ·  round(x, places)"
        );
        assert_eq!(function("min").unwrap().signature(), "min(a, b, …)");
        assert_eq!(
            function("apy").unwrap().signature(),
            "apy(apr, compounds)  ·  ear(apr, compounds)"
        );
        assert_eq!(
            function("factorial").unwrap().signature(),
            "factorial(x)  ·  x!"
        );
    }

    #[test]
    fn arity_descriptions_read_from_the_bounds() {
        let describe = |name: &str| function(name).unwrap().arity_description(name);
        assert_eq!(describe("sqrt"), "sqrt takes exactly 1 argument");
        assert_eq!(describe("mod"), "mod takes exactly 2 arguments");
        assert_eq!(describe("round"), "round takes 1 or 2 arguments");
        assert_eq!(describe("log"), "log takes 1 or 2 arguments");
        assert_eq!(describe("min"), "min takes at least 1 argument");
    }

    #[test]
    fn a_base_counts_toward_the_accepted_argument_count() {
        let log = function("log").unwrap();
        assert!(log.accepts(1));
        assert!(log.accepts(2), "log_b(x) arrives as two arguments");
        assert!(!log.accepts(3));

        let sqrt = function("sqrt").unwrap();
        assert!(sqrt.accepts(1));
        assert!(!sqrt.accepts(2), "sqrt takes no base");
    }
}
