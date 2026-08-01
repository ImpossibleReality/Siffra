use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::representations::{Value, install_static_rates};

fn display_value(value: Value) -> String {
    let (value, dimension) = value.into_parts();
    let units = dimension.to_string();
    if units.is_empty() {
        value.to_string()
    } else {
        format!("{} {}", value, units)
    }
}

fn one(line: &str) -> String {
    // A fixed rate keeps these tests deterministic and offline.
    install_static_rates(&[("USD", 1.08)]);
    match evaluate_line(line, &mut SiffraState::new()) {
        Ok(Some(value)) => display_value(value),
        Ok(None) => "<none>".to_string(),
        Err(error) => format!("ERR: {}", error.message()),
    }
}

#[test]
fn percent_is_a_named_value_with_scalar_conversions() {
    assert_eq!(one("5%"), "5 %");
    assert_eq!(one("(2 + 3)%"), "5 %");
    assert_eq!(one("5% as scalar"), "0.05");
    assert_eq!(one("5% as #"), "0.05");
    assert_eq!(one("5 percent as scalar"), "0.05");
    assert_eq!(one("0.05 as %"), "5 %");
    assert_eq!(one("0.05 as percent"), "5 %");
    assert_eq!(one("0.05 as #"), "0.05");
    assert_eq!(one("5 #"), "5");

    assert_eq!(one("5000 ppm as percent"), "0.5 %");
}

#[test]
fn multiplication_and_division_consume_the_percentage_unit() {
    assert_eq!(one("5% * $10"), "0.5 USD");
    assert_eq!(one("$10 * 5%"), "0.5 USD");
    assert_eq!(one("$10 / 5%"), "200 USD");
    assert_eq!(one("5% / 10"), "0.005");
    assert_eq!(one("5% / 10%"), "0.5");
}

#[test]
fn addition_and_subtraction_are_relative_and_ordered() {
    assert_eq!(one("$10 + 5%"), "10.5 USD");
    assert_eq!(one("$10 - 5%"), "9.5 USD");
    assert_eq!(one("10 + 5%"), "10.5");
    assert_eq!(one("10 - 5%"), "9.5");
    assert_eq!(one("10% + 5%"), "15 %");

    // A leading percentage is not reinterpreted relative to the value on its
    // right.
    assert!(one("5% + $10").starts_with("ERR:"));
    assert!(one("5% + 10").starts_with("ERR:"));
}

#[test]
fn natural_language_percentage_operations() {
    assert_eq!(one("20% of $10"), "2 USD");
    assert_eq!(one("5% on $30"), "31.5 USD");
    assert_eq!(one("6% off 40 EUR"), "37.6 EUR");

    assert_eq!(one("$50 as a % of $100"), "50 %");
    assert_eq!(one("$70 as a % on $20"), "250 %");
    assert!(
        one("$20 as a % off $70").starts_with("71.428571428571"),
        "got {}",
        one("$20 as a % off $70")
    );

    assert_eq!(one("5% of what is 6 EUR"), "120 EUR");
    assert!(
        one("5% on what is 6 EUR").starts_with("5.714285714285"),
        "got {}",
        one("5% on what is 6 EUR")
    );
    assert!(
        one("5% off what is 6 EUR").starts_with("6.315789473684"),
        "got {}",
        one("5% off what is 6 EUR")
    );
}

#[test]
fn chained_percentage_applications_compound_from_right_to_left() {
    assert_eq!(one("5% on 2% on $5"), "5.355 USD");
    assert_eq!(one("5% on (2% on $5)"), "5.355 USD");
    assert_eq!(one("5% on 2% off $5"), "5.145 USD");
}

#[test]
fn percentage_edge_cases_are_deliberate() {
    assert_eq!(one("-5% on $100"), "95 USD");
    assert_eq!(one("150% off 40 EUR"), "-20 EUR");
    assert_eq!(one("$10 + 0%"), "10 USD");

    assert!(one("0% of what is 6 EUR").starts_with("ERR: Percentage Error"));
    assert!(one("100% off what is 6 EUR").starts_with("ERR: Percentage Error"));
    assert!(one("$5 as a % of $0").starts_with("ERR: Percentage Error"));
    assert!(one("5 on $30").starts_with("ERR: Percentage Error"));
    assert!(one("5 m%").starts_with("ERR: Percentage Error"));
    assert!(one("5 m as a % of 2 s").starts_with("ERR: Unit Error"));
}
