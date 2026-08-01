use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::representations::Value;

fn display_value(value: Value) -> String {
    let (value, dimension) = value.into_parts();
    let units = dimension.to_string();
    if units.is_empty() {
        value.to_string()
    } else {
        format!("{value} {units}")
    }
}

fn one(line: &str) -> String {
    match evaluate_line(line, &mut SiffraState::new()) {
        Ok(Some(value)) => display_value(value),
        Ok(None) => "<none>".to_string(),
        Err(error) => format!("ERR: {}", error.message()),
    }
}

#[test]
fn scientific_and_decimal_literals_do_not_turn_into_expressions() {
    assert_eq!(one("1e-60"), "1E-60");
    assert_eq!(one("1E+5"), "100,000");
    assert_eq!(one(".5"), "0.5");
}

#[test]
fn commas_are_unambiguous_and_grouping_is_validated() {
    assert_eq!(one("min(1,2)"), "1");
    assert_eq!(one("min(1,234, 5)"), "5");
    assert_eq!(one("1,234,567"), "1,234,567");
    assert!(one("1,23").starts_with("ERR:"));
}

#[test]
fn transcendental_functions_enforce_and_convert_units() {
    assert_eq!(one("sin(90 deg)"), "1");
    assert_eq!(one("sin(100 grad)"), "1");
    assert_eq!(one("cos(1 rev)"), "1");
    assert!(one("sin(5 m)").starts_with("ERR: Unit Error"));
    assert!(one("ln(2 kg)").starts_with("ERR: Unit Error"));
    assert!(one("exp(2 s)").starts_with("ERR: Unit Error"));
    assert!(one("atan(1)").ends_with(" rad"));
    assert!(one("atan2(1 m, 1 m)").ends_with(" rad"));
}

#[test]
fn stoichiometric_bridge_matches_every_term_and_power() {
    assert!(one("1 [g H2O] in [mol H2O * m]").starts_with("ERR: Unit Error"));
    assert!(one("1 [g H2O^2] in [mol H2O]").starts_with("ERR: Unit Error"));
    assert_eq!(one("18.01528 g H2O in mol H2O"), "1 mol H2O");
}

#[test]
fn corrected_unit_factors_keep_display_precision_honest() {
    assert_eq!(one("1 lbmol in mol"), "453.59237 mol");
    assert!(
        one("180 deg in rad").starts_with("3.141592653589793238462643383"),
        "got {}",
        one("180 deg in rad")
    );
    assert_eq!(one("gamma(0.5)"), "1.7724538509055");
}

#[test]
fn tiny_inexact_results_and_zero_integer_arguments_work() {
    assert!(one("exp(-200)").starts_with("1.383896526736"));
    assert_eq!(one("sin(1e-60)"), "1E-60");
    assert_eq!(one("round(1.234, 0)"), "1");
    assert_eq!(one("choose(5, 0)"), "1");
    assert_eq!(one("permute(5, 0)"), "1");
    assert_eq!(one("gcd(0, 6)"), "6");
}

#[test]
fn negative_exponents_and_real_domains_are_handled_explicitly() {
    assert_eq!(one("2^-3"), "0.125");
    assert!(one("0^-1").starts_with("ERR: Domain Error"));
    assert!(one("sqrt(-4)").starts_with("ERR: Domain Error"));
    assert!(one("asin(2)").starts_with("ERR: Domain Error"));
    assert!(one("log(-5)").starts_with("ERR: Domain Error"));
    assert_eq!(one("root_3(-27)"), "-3");
    assert!(one("(-8)^(1/3)").starts_with("ERR: Domain Error"));
}

#[test]
fn displayed_temperature_and_fractional_powers_parse_again() {
    assert_eq!(one("5 °C"), "5 °C");
    let displayed = one("(2 m)^0.5");
    assert!(displayed.ends_with(" m^0.5"), "got {displayed}");
    assert_eq!(one(&displayed), displayed);
    assert_eq!(one("1 [m^0.5]"), "1 m^0.5");
    let inverse = one("avogadro");
    assert_eq!(one(&inverse), inverse);
}
