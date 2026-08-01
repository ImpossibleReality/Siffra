//! Currency (dynamic-unit) conversion. Rates are injected statically so the
//! suite stays offline and deterministic; the live Frankfurter fetch + disk
//! cache is exercised only at runtime.

use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::representations::{Value, install_static_rates};

fn display_value(val: Value) -> String {
    let (val, dim) = val.into_parts();
    let mut output = val.to_string();
    let dim_string = dim.to_string();
    if !dim_string.is_empty() {
        output.push(' ');
        output.push_str(&dim_string);
    }
    output
}

fn one(line: &str) -> String {
    let mut state = SiffraState::new();
    match evaluate_line(line, &mut state) {
        Ok(Some(v)) => display_value(v),
        Ok(None) => "<none>".to_string(),
        Err(e) => format!("ERR: {}", e.message()),
    }
}

/// A single test keeps the shared global rate table race-free under parallel
/// test execution. `CHF` is intentionally *not* installed, to exercise the
/// missing-rate path.
#[test]
fn currency_conversion_arithmetic_and_collapse() {
    install_static_rates(&[("USD", 1.08), ("GBP", 0.85), ("JPY", 160.0)]);

    // Rates are expressed relative to EUR.
    assert_eq!(one("100 EUR in USD"), "108 USD");
    assert!(one("100 USD in EUR").starts_with("92.59"));
    assert!(one("100 USD in GBP").starts_with("78.70"));
    assert_eq!(one("1000 JPY in USD"), "6.75 USD");

    assert_eq!(one("2 USD * 3"), "6 USD");
    assert_eq!(one("10 USD / 4 USD"), "2.5");
    assert_eq!(one("5 USD + 3 EUR"), "8.24 USD");

    assert_eq!(one("10 USD/h * 30 min"), "5 USD");

    // Unknown codes and known currencies without rates fail gracefully.
    assert!(one("100 XYZ in USD").starts_with("ERR"));
    assert!(one("100 CHF in USD").starts_with("ERR"));

    // Invalid rates are unavailable rather than silently becoming infinity.
    install_static_rates(&[("USD", f64::NAN)]);
    assert!(one("1 USD in EUR").starts_with("ERR:"));
}
