use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::representations::Value;

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
    match evaluate_line(line, &mut SiffraState::new()) {
        Ok(Some(value)) => display_value(value),
        Ok(None) => "<none>".to_string(),
        Err(error) => format!("ERR: {}", error.message()),
    }
}

#[test]
fn rounding_functions_preserve_units() {
    assert_eq!(one("round(12.345, 2)"), "12.35");
    assert_eq!(one("round(1250, -2)"), "1,300");
    assert_eq!(one("round(-1.5)"), "-2");
    assert_eq!(one("round(12.345 m, 2)"), "12.35 m");
    assert_eq!(one("floor(-1.2 kg)"), "-2 kg");
    assert_eq!(one("ceil(-1.2 kg)"), "-1 kg");

    assert!(one("round(1.2, 1.5)").starts_with("ERR: Round Error"));
    assert!(one("round(1.2, 2 m)").starts_with("ERR: Unit Error"));
}

#[test]
fn comparisons_and_mean_convert_compatible_units() {
    assert_eq!(one("min(2 m, 150 cm, 3 m)"), "1.5 m");
    assert_eq!(one("max(2 m, 150 cm, 3 m)"), "3 m");
    assert_eq!(one("clamp(250 cm, 1 m, 2 m)"), "200 cm");
    assert_eq!(one("mean(1 m, 200 cm, 3 m)"), "2 m");

    assert!(one("min(1 m, 1 s)").starts_with("ERR: Unit Error"));
    assert!(one("clamp(5, 10, 1)").starts_with("ERR: Clamp Error"));
}

#[test]
fn geometry_helpers_are_unit_aware() {
    assert_eq!(one("hypot(3 m, 400 cm)"), "5 m");
    assert!(
        one("atan2(1, 1)").starts_with("0.785398163397"),
        "got {}",
        one("atan2(1, 1)")
    );
    assert!(
        one("atan2(1 m, -1 m)").starts_with("2.356194490192"),
        "got {}",
        one("atan2(1 m, -1 m)")
    );
    assert!(one("atan2(0, 0)").starts_with("ERR: Domain Error"));
    assert!(one("hypot(3 m, 4 s)").starts_with("ERR: Unit Error"));
}

#[test]
fn apy_accepts_percentage_or_decimal_apr() {
    assert!(
        one("apy(5%, 12)").starts_with("5.116189788"),
        "got {}",
        one("apy(5%, 12)")
    );
    assert!(
        one("apy(0.05, 12)").starts_with("0.05116189788"),
        "got {}",
        one("apy(0.05, 12)")
    );
    assert_eq!(one("apy(5%, 1)"), "5 %");

    assert!(one("apy(5 USD, 12)").starts_with("ERR: Unit Error"));
    assert!(one("apy(5%, 0)").starts_with("ERR: APY Error"));
    assert!(one("apy(5%, 12.5)").starts_with("ERR: APY Error"));
}

#[test]
fn golden_ratio_constant_is_available() {
    assert!(
        one("phi").starts_with("1.618033988749"),
        "got {}",
        one("phi")
    );
}

#[test]
fn remaining_numeric_helpers_handle_units_and_signs() {
    assert_eq!(one("trunc(1.9 kg)"), "1 kg");
    assert_eq!(one("trunc(-1.9 kg)"), "-1 kg");
    assert_eq!(one("sign(-42 m)"), "-1");
    assert_eq!(one("sign(0 USD)"), "0.0");
    assert_eq!(one("mod(5 m, 200 cm)"), "1 m");
    assert_eq!(one("mod(-5, 3)"), "1");

    assert!(one("mod(5 m, 2 s)").starts_with("ERR: Unit Error"));
    assert!(one("mod(5, 0)").starts_with("ERR: Division Error"));
}

#[test]
fn integer_and_combinatoric_helpers_validate_their_domains() {
    assert_eq!(one("gcd(84, 30)"), "6");
    assert_eq!(one("gcd(-84, 30)"), "6");
    assert_eq!(one("lcm(84, 30)"), "420");
    assert_eq!(one("choose(10, 3)"), "120");
    assert_eq!(one("permute(10, 3)"), "720");

    assert!(one("gcd(4.5, 3)").starts_with("ERR: Integer Error"));
    assert!(one("choose(3, 4)").starts_with("ERR: Domain Error"));
    assert!(one("permute(5 m, 2)").starts_with("ERR: Unit Error"));
}

#[test]
fn hyperbolic_and_gamma_functions_are_available() {
    assert_eq!(one("sinh(0)"), "0.0");
    assert_eq!(one("cosh(0)"), "1");
    assert_eq!(one("tanh(0)"), "0.0");
    assert!(
        one("sinh(1)").starts_with("1.175201193643"),
        "got {}",
        one("sinh(1)")
    );
    assert_eq!(one("gamma(5)"), "24");
    assert!(
        one("gamma(0.5)").starts_with("1.772453850905"),
        "got {}",
        one("gamma(0.5)")
    );

    assert!(one("sinh(1 m)").starts_with("ERR: Unit Error"));
    assert!(one("gamma(0)").starts_with("ERR: Gamma Error"));
}

#[test]
fn finance_helpers_preserve_value_units() {
    assert_eq!(one("apr(apy(5%, 12), 12)"), "5 %");
    assert!(
        one("ear(5%, 12)").starts_with("5.116189788"),
        "got {}",
        one("ear(5%, 12)")
    );
    assert!(
        one("fv(1000 USD, 5%, 10)").starts_with("1,628.894626"),
        "got {}",
        one("fv(1000 USD, 5%, 10)")
    );
    assert!(
        one("compound(1000 USD, 5%, 10)").starts_with("1,628.894626"),
        "got {}",
        one("compound(1000 USD, 5%, 10)")
    );
    assert!(
        one("pv(fv(1000 USD, 5%, 10), 5%, 10)").starts_with("1,000"),
        "got {}",
        one("pv(fv(1000 USD, 5%, 10), 5%, 10)")
    );
    assert!(
        one("pmt(1000 USD, 5%, 12)").starts_with("112.825410"),
        "got {}",
        one("pmt(1000 USD, 5%, 12)")
    );
    assert_eq!(one("pmt(1200 USD, 0%, 12)"), "100 USD");

    assert!(one("fv(1000 USD, 5 m, 10)").starts_with("ERR: Unit Error"));
    assert!(one("pmt(1000 USD, 5%, 0)").starts_with("ERR: Finance Error"));
    assert!(one("fv(1000 USD, -100%, 10)").starts_with("ERR: Finance Error"));
    assert!(one("apr(-100%, 12)").starts_with("ERR: APR Error"));
    assert!(one("apy(-1200%, 12)").starts_with("ERR: APY Error"));
}

#[test]
fn simplify_collapses_compound_units_to_named() {
    assert_eq!(one("simplify(1000 kg*m/s^2)"), "1,000 N");
    assert_eq!(one("simplify(1 kg*m^2/s^2)"), "1 J");
    assert_eq!(one("simplify(1 J/s)"), "1 W");
    assert_eq!(one("simplify(5 N*m)"), "5 J");
    assert_eq!(one("simplify(1 / (1 s))"), "1 Hz");
    assert_eq!(one("simplify(1 kWh)"), "3,600,000 J");
    assert_eq!(one("simplify(5 m/s)"), "5 m/s");
    assert_eq!(one("simplify(60 mph)"), "60 mph");
    assert_eq!(one("simplify(42)"), "42");

    assert!(one("simplify(1, 2)").starts_with("ERR: Argument Error"));
}

#[test]
fn expand_unfolds_named_units_into_base_units() {
    assert_eq!(one("expand(5 N)"), "5 kg*m/s^2");
    assert_eq!(one("expand(1 J)"), "1 kg*m^2/s^2");
    assert_eq!(one("expand(1 W)"), "1 kg*m^2/s^3");
    assert_eq!(one("expand(1 kWh)"), "3,600,000 kg*m^2/s^2");
    assert_eq!(one("expand(5 m/s)"), "5 m/s");
    assert_eq!(one("expand(42)"), "42");

    assert!(one("expand(1, 2)").starts_with("ERR: Argument Error"));
}

#[test]
fn simplify_and_expand_round_trip() {
    assert_eq!(one("simplify(expand(1 N))"), "1 N");
    assert_eq!(one("simplify(expand(1 W))"), "1 W");
}

#[test]
fn physical_constants_include_dimensions() {
    assert_eq!(one("lightspeed"), "299,792,458 m/s");
    assert_eq!(one("standardgravity"), "9.80665 m/s^2");
    assert_eq!(one("avogadro"), "6.02214076E23 1/mol");
    assert_eq!(one("planck"), "6.62607015E-34 J*s");
    assert_eq!(one("boltzmann"), "1.380649E-23 J/iK");
    assert_eq!(one("elementarycharge"), "1.602176634E-19 C");

    assert_eq!(one("lightspeed * 1 s in km"), "299,792.458 km");
    assert_eq!(one("standardgravity * 1 s^2"), "9.80665 m");
    assert_eq!(one("planck * 1 Hz in J"), "6.62607015E-34 J");
    assert_eq!(one("boltzmann * (300 iK) in J"), "4.141947E-21 J");
}

#[test]
fn a_base_is_only_accepted_by_the_functions_that_read_one() {
    assert_eq!(one("log_2(32)"), "5");
    assert_eq!(one("root_3(27)"), "3");
    // The same call written out long-hand, since the base is the first argument.
    assert_eq!(one("root(3, 27)"), "3");

    assert_eq!(one("sin_2(1)"), "ERR: Argument Error");
    assert_eq!(one("abs2(4)"), "ERR: Argument Error");
}

#[test]
fn aliases_resolve_to_the_same_function() {
    assert_eq!(one("ear(5%, 12)"), one("apy(5%, 12)"));
    assert_eq!(
        one("compound(1000 USD, 5%, 10)"),
        one("fv(1000 USD, 5%, 10)")
    );
}

#[test]
fn argument_counts_are_checked_before_the_function_runs() {
    assert_eq!(one("sqrt(1, 2)"), "ERR: Argument Error");
    assert_eq!(one("round(1, 2, 3)"), "ERR: Argument Error");
    assert_eq!(one("mod(5)"), "ERR: Argument Error");
    assert_eq!(one("nosuchfunction(1)"), "ERR: Name Error");
}
