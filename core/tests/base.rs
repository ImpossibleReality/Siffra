//! End-to-end checks for parser and evaluator behavior.

use siffra::evaluation::{evaluate_line, SiffraState};
use siffra::representations::Value;

fn display_value(val: Value) -> String {
    let (val, dim) = val.into_parts();
    let mut output = String::new();
    output.push_str(&val.to_string());
    let dim_string = dim.to_string();
    if !dim_string.is_empty() {
        output.push(' ');
        output.push_str(&dim_string);
    }
    output
}

/// Evaluate a block of lines with shared state and return the
/// rendered output for each line that produced a value.
fn run(block: &str) -> Vec<String> {
    let mut state = SiffraState::new();
    let mut out = Vec::new();
    for line in block.lines() {
        match evaluate_line(line, &mut state) {
            Ok(Some(v)) => out.push(display_value(v)),
            Ok(None) => {}
            Err(e) => out.push(format!("ERR: {}", e.message())),
        }
    }
    out
}

fn one(line: &str) -> String {
    let mut state = SiffraState::new();
    match evaluate_line(line, &mut state) {
        Ok(Some(v)) => display_value(v),
        Ok(None) => "<none>".to_string(),
        Err(e) => format!("ERR: {} ({:?})", e.message(), e.description()),
    }
}

#[test]
fn simple_calculations() {
    assert_eq!(one("5 + 2 / 4 - 2"), "3.5");
    assert_eq!(one("5 ^ log_(2+3)(100)"), "100");
    assert_eq!(one("3E100 / 4E62"), "7.5E37");
}

#[test]
fn implicit_multiplication_has_lower_precedence_than_exponentiation() {
    // No space means an adjacent variable; a space selects the unit grammar,
    // even when a variable shares the unit's name.
    let out = run("m = 1\n5m^2\n5 m^2");
    assert_eq!(out, vec!["1", "5", "5 m^2"]);

    // Exponents bind to their adjacent term, while parentheses can opt into
    // raising the entire product. Chained exponents remain right-associative.
    let out = run("x = 2\n2x^3\n(2x)^3\n2x^2^3\n2x^-2\n2x^2 / 4");
    assert_eq!(out, vec!["2", "16", "64", "512", "0.5", "2"]);

    // Factorial binds before exponentiation and implicit multiplication.
    let out = run("x = 3\n2x!\n2(x!)^2");
    assert_eq!(out, vec!["3", "12", "72"]);
}

#[test]
fn dimensional_analysis_block() {
    let out = run("accel = 5 m/s^2\nvel = (accel * 2 s) in km/h\npos = (vel * 100 ms) in m");
    assert_eq!(out, vec!["5 m/s^2", "36 km/h", "1 m"]);
}

#[test]
fn dimensional_addition() {
    assert_eq!(one("5 m + 3 in"), "5.0762 m");
}

#[test]
fn natural_language() {
    assert_eq!(one("5 times 20 plus 2"), "102");
    assert_eq!(one("10% of 20"), "2");
}

#[test]
fn variables_and_prev_total() {
    let out = run("2\n3\ntotal");
    assert_eq!(out, vec!["2", "3", "5"]);

    let out = run("10\nprev + 5");
    assert_eq!(out, vec!["10", "15"]);
}

#[test]
fn functions() {
    assert_eq!(one("sqrt(16)"), "4");
    assert_eq!(one("log(100)"), "2");
    assert_eq!(one("log2(8)"), "3");
}

#[test]
fn factorial() {
    assert_eq!(one("5!"), "120");
    assert_eq!(one("0!"), "1");
    assert_eq!(one("10!"), "3,628,800");
    assert!(one("0.5!").starts_with("0.886"), "got {}", one("0.5!"));
    assert!(one("1001!").starts_with("ERR: Factorial Error"));
    assert!(one("(-1000.5)!").starts_with("ERR: Factorial Error"));
    assert!(one("(-1)!").starts_with("ERR: Factorial Error"));
}

#[test]
fn inexact_functions_round_without_erasing_small_magnitudes() {
    assert_eq!(one("sin 2pi"), "0.0");
    assert_eq!(one("cos 2pi"), "1");
    assert_eq!(one("exp(0)"), "1");
    assert!(one("exp(-200)").starts_with("1.383896526736"));
    assert_eq!(one("sin(1e-60)"), "1E-60");
}

#[test]
fn root() {
    assert_eq!(one("root_3(27)"), "3");
    assert_eq!(one("root(3, 27)"), "3");
    assert_eq!(one("root_4(16)"), "2");
    assert_eq!(one("root(16)"), "4");
}

#[test]
fn fractional_powers_do_not_hang() {
    // Fractional powers that land on exact integers must complete promptly.
    assert_eq!(one("27 ^ (1/3)"), "3");
    assert_eq!(one("2 ^ 0.5").starts_with("1.4142").to_string(), "true");
    assert_eq!(one("8 ^ (1/3)"), "2");
    assert_eq!(one("100 ^ 0.5"), "10");
}

#[test]
fn parse_errors_are_specific() {
    let e = evaluate_line("5 +", &mut SiffraState::new()).unwrap_err();
    assert!(
        e.message().contains("Incomplete"),
        "msg was {}",
        e.message()
    );
    assert!(e.span().is_some());

    let e = evaluate_line("5 @ 3", &mut SiffraState::new()).unwrap_err();
    assert!(
        e.message().contains("Unexpected"),
        "msg was {}",
        e.message()
    );
    assert!(e.span().is_some());
}

#[test]
fn stoichiometry() {
    let oxygen = one("5 g O2 in mol O2");
    assert!(oxygen.starts_with("0.156"), "got {oxygen}");

    let glucose = one("5 mol C6H12O6 in g");
    assert!(glucose.starts_with("900.7"), "got {glucose}");

    // Brackets support compound expressions as well as a single qualified unit.
    let out = one("18 [g H2O] as [mol H2O]");
    assert!(out.starts_with("0.99") || out.starts_with("1"), "got {out}");
}

#[test]
fn bracketed_units_allow_spaces_and_literal_one() {
    assert_eq!(one("6 [m / s] * 2 s"), "12 m");
    assert_eq!(one("1 [1/s] in Hz"), "1 Hz");
    assert_eq!(one("5 [s/1]"), "5 s");
}

#[test]
fn conversion_target_stops_before_spaced_arithmetic() {
    let output = one("190 deg as rad  *2");
    assert!(output.starts_with("6.632"), "got {output}");
    assert!(output.ends_with(" rad"), "got {output}");
}

#[test]
fn derived_units_round_trip_and_compose() {
    // Named units keep their display; base quotients retain built-up units.
    assert_eq!(one("60 mph"), "60 mph");
    assert_eq!(one("100 km / 2 h"), "50 km/h");
    assert_eq!(one("2 Hz"), "2 Hz");
}

#[test]
fn torque_and_energy_do_not_collapse_into_each_other() {
    // Equal dimensionality does not erase deliberate display units.
    assert_eq!(one("5 N * 3 m"), "15 N*m");
    assert_eq!(one("5 J"), "5 J");
    assert!(one("12 N in J").starts_with("ERR"), "got {}", one("12 N in J"));
}

#[test]
fn explicit_conversion_collapses_to_named_units() {
    // Named collapse is explicit; bare products retain their built-up units.
    assert_eq!(one("100 kg * 9.8 m/s^2"), "980 kg*m/s^2");
    assert_eq!(one("100 kg * 9.8 m/s^2 in N"), "980 N");
    assert_eq!(one("10 W * 5 s in J"), "50 J");
}

#[test]
fn scalar_units_scale_a_bare_number() {
    // A scalar unit converts a plain number rather than merely labelling it.
    assert_eq!(one("0.5 as percent"), "50 %");
}

#[test]
fn absolute_temperature_converts_affinely() {
    // Absolute temperatures use scale and offset, not just a ratio.
    assert_eq!(one("100 C in F"), "212 °F");
    assert_eq!(one("0 C in K"), "273.15 K");
    assert_eq!(one("32 F in C"), "0.0 °C");
    assert_eq!(one("37 C in F"), "98.6 °F");
    assert_eq!(one("300 K in C"), "26.85 °C");
}

#[test]
fn absolute_temperature_decays_under_arithmetic() {
    // Arithmetic decays absolute temperatures to linear intervals.
    assert_eq!(one("20 C + 5 C"), "25 °C");
}

#[test]
fn products_collapse_when_an_axis_fully_cancels() {
    assert_eq!(one("5 mph * 2 h"), "10 mi");
    assert_eq!(one("5 mph * 30 min"), "2.5 mi");
    assert_eq!(one("10 J / 5 N"), "2 m");
    assert_eq!(one("5 N * 3 m"), "15 N*m");
    assert_eq!(one("6 m / 2 s"), "3 m/s");
}
