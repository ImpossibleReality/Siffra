use siffra::evaluation::{evaluate_line, SiffraState};

fn error(line: &str) -> (String, String) {
    let error = evaluate_line(line, &mut SiffraState::new()).expect_err("line should fail");
    (
        error.message(),
        error
            .description()
            .expect("error should have a description"),
    )
}

#[test]
fn arithmetic_and_conversion_errors_name_the_units() {
    assert_eq!(error("1 m + 1 s").1, "Cannot add m and s");
    assert_eq!(error("1 kg - 1 USD").1, "Cannot subtract kg and USD");
    assert_eq!(error("1 m in s").1, "Cannot convert m to s");
}

#[test]
fn function_errors_name_actual_and_expected_units() {
    assert_eq!(error("min(1 m, 1 s)").1, "min cannot mix m and s");
    assert_eq!(error("ln(2 kg)").1, "ln must be unitless; got kg");
    assert_eq!(
        error("sin(5 m)").1,
        "sin requires an angle or unitless input; got m"
    );
}

#[test]
fn variable_errors_name_the_variable_and_retain_the_cause() {
    let mut state = SiffraState::new();
    let original =
        evaluate_line("distance = 1 m + 1 s", &mut state).expect_err("definition should fail");
    assert_eq!(
        original.description().as_deref(),
        Some("Cannot add m and s")
    );

    let propagated =
        evaluate_line("distance + 1", &mut state).expect_err("variable use should fail");
    assert_eq!(propagated.message(), "Variable Error");
    assert_eq!(
        propagated.description().as_deref(),
        Some("Variable 'distance' is invalid: Cannot add m and s")
    );

    assert_eq!(error("missing + 1").1, "Variable 'missing' not found");
}

#[test]
fn unknown_denominator_units_are_named() {
    let error = error("1 [m / noSuchUnit]");
    assert_eq!(error.0, "Unit Error");
    assert_eq!(error.1, "Unit 'noSuchUnit' not defined");
}

#[test]
fn total_errors_name_the_units_that_broke_the_total() {
    let mut state = SiffraState::new();
    evaluate_line("1 m", &mut state).expect("first value should succeed");
    evaluate_line("1 s", &mut state).expect("second value should succeed");

    let error = evaluate_line("total", &mut state).expect_err("total should fail");
    assert_eq!(error.message(), "Total Error");
    assert_eq!(
        error.description().as_deref(),
        Some("Block total cannot mix m and s")
    );
}
