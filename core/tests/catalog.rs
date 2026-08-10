//! The documentation is generated from the definitions, so what is left to
//! check is that it tells the truth: every documented example is evaluated here
//! and compared against the result the docs show.
//!
//! A documented result may be rounded — `pi` is written `3.141593`, not to the
//! last digit the engine carries — so an example passes when it matches the
//! real result either exactly or once that result is rounded to as many decimal
//! places as the documentation shows. Units and thousands separators still have
//! to match exactly.

use siffra::catalog::{CatalogExample, catalog};
use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::representations::{FormattedValue, Value, format_value};

fn evaluate(input: &str) -> Result<Value, String> {
    let mut state = SiffraState::new();
    match evaluate_line(input, &mut state) {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err("produced no result".to_string()),
        Err(error) => Err(format!(
            "{}: {}",
            error.message(),
            error.description().unwrap_or_default()
        )),
    }
}

fn rendered(value: &Value) -> String {
    format_value(value, Some(SiffraState::new().timezone())).to_plain_string()
}

/// How many decimal places a documented result shows, if it is a number at all.
fn documented_decimals(documented: &str) -> Option<usize> {
    let number = documented.split(' ').next()?;
    match number.split_once('.') {
        Some((_, fraction)) => Some(fraction.chars().take_while(char::is_ascii_digit).count()),
        None => number.chars().any(|c| c.is_ascii_digit()).then_some(0),
    }
}

fn check(subject: &str, examples: &[CatalogExample], failures: &mut Vec<String>) {
    for example in examples {
        let value = match evaluate(&example.input) {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!("{subject}: `{}` failed: {error}", example.input));
                continue;
            }
        };

        let actual = rendered(&value);
        if actual == example.result {
            continue;
        }

        // Preprocess documented result
        let mut result = example.result.clone();
        if result.starts_with("$") {
            result = format!("{} USD", &result[1..]);
        }

        // Only plain numbers can be re-rounded; a date is already exact or wrong.
        let rounded = matches!(
            format_value(&value, Some(SiffraState::new().timezone())),
            FormattedValue::Number { .. }
        )
        .then(|| documented_decimals(&result))
        .flatten()
        .map(|places| {
            rendered(&Value::new(
                value.value().round_decimal_places(places as i32),
                Some(value.dimension()),
            ))
        });

        if rounded.as_deref() != Some(result.as_str()) {
            failures.push(format!(
                "{subject}: `{}` gave `{actual}`, documented as `{}`",
                example.input, result
            ));
        }
    }
}

#[test]
fn every_documented_example_evaluates_to_its_documented_result() {
    let catalog = catalog();
    let mut failures = Vec::new();

    for function in &catalog.functions {
        check(&function.name, &function.examples, &mut failures);
    }
    for constant in &catalog.constants {
        check(&constant.name, &constant.examples, &mut failures);
    }

    assert!(
        failures.is_empty(),
        "documented examples disagree with the implementation:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_function_and_constant_is_documented() {
    let catalog = catalog();

    for function in &catalog.functions {
        let name = &function.name;
        assert!(!function.category.is_empty(), "{name} has no category");
        assert!(
            !function.description.is_empty(),
            "{name} has no description"
        );
        assert!(
            function.signature.starts_with(name.as_str()),
            "{name} has a signature that does not start with its name: {}",
            function.signature
        );
        assert!(!function.examples.is_empty(), "{name} has no examples");
    }

    for constant in &catalog.constants {
        assert!(
            !constant.description.is_empty(),
            "{} has no description",
            constant.name
        );
        assert!(
            !constant.examples.is_empty(),
            "{} has no examples",
            constant.name
        );
    }
}
