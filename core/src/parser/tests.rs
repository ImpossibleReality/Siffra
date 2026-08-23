use super::grammar::KNOWN_FUNCS;
use super::*;

fn parse(s: &str) -> ParsedLine {
    parse_line(s).unwrap_or_else(|e| panic!("failed to parse {:?}: {:?}", s, e.message()))
}

#[test]
fn syntax_errors_are_described_by_kind() {
    let describe = |input: &str| {
        let error = parse_line(input).expect_err("line should fail");
        (error.message(), error.description().expect("description"))
    };

    assert_eq!(
        describe("(5 + 2"),
        (
            "Unclosed bracket".to_string(),
            "Missing a closing ')'".to_string()
        )
    );
    assert_eq!(
        describe("5 *"),
        (
            "Incomplete expression".to_string(),
            "Expected something after '*'".to_string()
        )
    );
    assert_eq!(
        describe("@#$"),
        (
            "Unexpected '@'".to_string(),
            "The character '@' isn't valid here".to_string()
        )
    );
}

#[test]
fn parses_simple_expression() {
    assert!(matches!(parse("5 + 2 / 4 - 2"), ParsedLine::Expression(_)));
}

#[test]
fn parses_variable_assignment() {
    assert!(matches!(parse("x = 5"), ParsedLine::Variable(name, _) if name == "x"));
    assert!(matches!(parse("let y = 5"), ParsedLine::Variable(name, _) if name == "y"));
}

#[test]
fn parses_comments() {
    assert!(matches!(parse("// a comment"), ParsedLine::Comment));
    assert!(matches!(parse("/* block */"), ParsedLine::Comment));
    assert!(matches!(parse("   "), ParsedLine::Comment));
}

#[test]
fn parses_headers() {
    assert!(matches!(parse("# Trip budget"), ParsedLine::Header(t) if t == "Trip budget"));
    assert!(matches!(parse("  ## Notes  "), ParsedLine::Header(t) if t == "Notes"));
    assert!(matches!(parse("#"), ParsedLine::Header(t) if t.is_empty()));
    assert!(matches!(parse("5 #"), ParsedLine::Expression(_)));
}

#[test]
fn parses_dimensional_numbers() {
    assert!(matches!(parse("5 m + 3 in"), ParsedLine::Expression(_)));
    assert!(matches!(
        parse("accel = 5 m/s^2"),
        ParsedLine::Variable(_, _)
    ));
}

#[test]
fn compound_units_allow_brackets_and_displayed_inverse_units() {
    for input in [
        "5 [kg * m / s^2]",
        "5 [1/s]",
        "5 [s/1]",
        "5 [ 1 / s ]",
        "5 1/s",
        "5 s in 1/s",
        "5 g O2 in mol O2",
        "5 mol C6H12O6 in g",
        "18 [g H2O] as [mol H2O]",
    ] {
        parse(input);
    }
}

#[test]
fn every_ungrouped_function_name_is_a_real_function() {
    // `sqrt 2` is only read as a call for the names listed here, so the list
    // has to keep naming functions that exist.
    for name in KNOWN_FUNCS {
        assert!(
            crate::registry::function(name).is_some(),
            "KNOWN_FUNCS names '{name}', which is not a defined function"
        );
    }
}

#[test]
fn parses_functions() {
    parse("sin(cos(5))");
    parse("log2(100)");
    parse("log_(2+3)(100)");
    parse("5 ^ log_(2+3)(100)");
    parse("log 100");
    parse("sqrt 2");
}

#[test]
fn parses_natural_language() {
    parse("5 times 20 plus 2");
    parse("10% of 20");
    parse("100 divided by 4");
}

#[test]
fn chained_percentage_applications_are_right_associative() {
    let ParsedLine::Expression(ParsedExpr::BinOp {
        lhs,
        op: Op::PercentageOn,
        rhs,
        ..
    }) = parse("5% on 2% on $5")
    else {
        panic!("outer percentage application should be parsed");
    };

    assert!(matches!(
        *lhs,
        ParsedExpr::UnOpPost {
            op: OpPost::Percent,
            ..
        }
    ));
    assert!(matches!(
        *rhs,
        ParsedExpr::BinOp {
            op: Op::PercentageOn,
            ..
        }
    ));
}

#[test]
fn parses_conversions() {
    parse("(5 m) in km");
    parse("5 m in km");
    parse("2 s as ms");
}

#[test]
fn conversion_binds_looser_than_multiplication_and_division() {
    for input in ["100 kg * 9.8 m/s^2 in N", "10 W * 5 s as J"] {
        let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) = parse(input) else {
            panic!("conversion should be the outer expression for {input:?}");
        };

        assert!(matches!(op, OpPost::Convert(_)));
        assert!(matches!(
            *lhs,
            ParsedExpr::BinOp {
                op: Op::Multiply,
                ..
            }
        ));
    }

    let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) = parse("100 km / 2 h in mph")
    else {
        panic!("conversion should be the outer expression for a quotient");
    };

    assert!(matches!(op, OpPost::Convert(_)));
    assert!(matches!(*lhs, ParsedExpr::BinOp { op: Op::Divide, .. }));
}

#[test]
fn arithmetic_can_follow_a_conversion() {
    let ParsedLine::Expression(ParsedExpr::BinOp {
        lhs,
        op: Op::Multiply,
        ..
    }) = parse("190 deg as rad  *2")
    else {
        panic!("multiplication should follow the completed conversion");
    };

    assert!(matches!(
        *lhs,
        ParsedExpr::UnOpPost {
            op: OpPost::Convert(_),
            ..
        }
    ));
}

#[test]
fn end_of_line_arrow_converts_the_whole_expression() {
    let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) = parse("5 m + 3 in -> cm")
    else {
        panic!("`->` should wrap the whole line expression");
    };
    assert!(matches!(op, OpPost::Convert(_)));
    assert!(matches!(*lhs, ParsedExpr::BinOp { op: Op::Add, .. }));

    parse("5 m + 3 in ->cm");
    let ParsedLine::Variable(name, ParsedExpr::UnOpPost { op, .. }) = parse("x = 5 m + 3 in -> cm")
    else {
        panic!("`->` on an assignment should convert the stored value");
    };
    assert_eq!(name, "x");
    assert!(matches!(op, OpPost::Convert(_)));
}

#[test]
fn arrow_is_only_valid_at_the_end_of_the_line() {
    assert!(parse_line("(5 -> cm) + 2").is_err());

    for input in ["5 m -> cm + 2", "5 m -> cm * 3", "5 m -> cm in mm"] {
        let error = parse_line(input).expect_err("content after `-> unit` is invalid");
        assert_eq!(
            error.description().as_deref(),
            Some("`-> [Unit]` must be at the end of a line")
        );
        let arrow = input.find("->").unwrap();
        assert_eq!(error.span(), Some((arrow, arrow + 2)));
    }
}

#[test]
fn parses_temporal_literals() {
    for input in [
        "2024-02-10",
        "2024-02-10 12:00",
        "2024-02-10 12:00:30",
        "2024-02-10 12:00 America/Los_Angeles",
        "2024-02-10 12:00 UTC",
        "2024-02-10 12:00 +05:30",
        "5pm UTC on feb 2 2026",
        "5pm UTC today",
        "5pm UTC",
        "5pm today",
        "now",
        "today + 1 day",
        "9:00",
        "17:00 - 9:15",
    ] {
        parse(input);
    }
}

#[test]
fn a_temporal_literal_after_a_conversion_keyword_makes_a_range() {
    let ParsedLine::Expression(ParsedExpr::BinOp {
        op: Op::Interval,
        lhs,
        rhs,
        ..
    }) = parse("2024-02-10 to 2024-03-10")
    else {
        panic!("a range should lower to an interval");
    };

    let month = |expr: &ParsedExpr| match expr {
        ParsedExpr::Instant { instant, .. } => instant.date.expect("a dated literal").month,
        other => panic!("expected an instant, got {other:?}"),
    };
    assert_eq!(month(&lhs), 2);
    assert_eq!(month(&rhs), 3);
}

#[test]
fn a_range_endpoint_may_be_any_expression_a_unit_cannot_claim() {
    for input in ["now to launch", "9:00 to (8:00 + 3 h)", "today to tomorrow"] {
        let ParsedLine::Expression(ParsedExpr::BinOp {
            op: Op::Interval, ..
        }) = parse(input)
        else {
            panic!("{input:?} should lower to an interval");
        };
    }
}

#[test]
fn written_calendar_shifts_lower_to_one_addition_or_subtraction() {
    for (input, expected_op) in [
        ("3 weeks after March 14, 2019", Op::Add),
        ("28 days before March 12", Op::Subtract),
        ("2 months 3 days after June 5", Op::Add),
        ("2 weeks from now", Op::Add),
        ("90 min after 5:00 PM", Op::Add),
    ] {
        let ParsedLine::Expression(ParsedExpr::BinOp { lhs, op, rhs, .. }) = parse(input) else {
            panic!("{input:?} should lower to a calendar shift");
        };

        assert_eq!(op, expected_op, "input was {input:?}");
        assert!(
            matches!(
                *lhs,
                ParsedExpr::Instant { .. } | ParsedExpr::ClockTime { .. }
            ),
            "the reference should be the left operand for {input:?}"
        );
        assert!(
            !matches!(
                *rhs,
                ParsedExpr::Instant { .. } | ParsedExpr::ClockTime { .. }
            ),
            "the duration should be the right operand for {input:?}"
        );
        if input == "2 months 3 days after June 5" {
            assert!(
                matches!(*rhs, ParsedExpr::Mixed { .. }),
                "a mixed duration should remain one expression until evaluation"
            );
        }
    }
}

#[test]
fn a_dimension_after_a_conversion_keyword_is_still_a_conversion() {
    for input in [
        "2024-02-10 to 2024-03-10 in days",
        "5 m in km",
        "3 in in cm",
    ] {
        let ParsedLine::Expression(expression) = parse(input) else {
            panic!("{input:?} should be an expression");
        };
        assert!(
            matches!(
                expression,
                ParsedExpr::UnOpPost {
                    op: OpPost::Convert(_),
                    ..
                }
            ),
            "{input:?} should end in a conversion"
        );
    }
}

#[test]
fn zone_names_never_claim_a_compound_unit() {
    parse("5 m in m/s");
    parse("100 km / 2 h in mph");
    assert!(parse_line("2024-02-10 12:00 Bogus/Zone").is_err());
}

#[test]
fn arrow_conversions_chain_but_stay_at_the_end_of_the_line() {
    let ParsedLine::Expression(ParsedExpr::UnOpPost { lhs, op, .. }) =
        parse("2024-02-10 to 2024-03-10 -> day -> hour")
    else {
        panic!("the last arrow should be the outer conversion");
    };
    assert!(matches!(op, OpPost::Convert(_)));
    assert!(matches!(
        *lhs,
        ParsedExpr::UnOpPost {
            op: OpPost::Convert(_),
            ..
        }
    ));

    assert!(parse_line("5 m -> cm -> mm + 2").is_err());
}

#[test]
fn a_shrinking_run_of_one_dimension_is_one_mixed_literal() {
    for (input, rungs) in [
        ("5 ft 2 in", 2),
        ("2 h 5 min 2 s", 3),
        ("6 lb 3 oz", 2),
        ("5'2\"", 2),
    ] {
        let ParsedLine::Expression(ParsedExpr::Mixed { rungs: parsed, .. }) = parse(input) else {
            panic!("{input:?} should be one mixed literal");
        };
        assert_eq!(parsed.len(), rungs, "input was {input:?}");
    }

    for input in ["2 m 5 kg", "5 m 2 m", "2 in 5 ft", "5 ft 2"] {
        assert!(
            !matches!(
                parse_line(input),
                Ok(ParsedLine::Expression(ParsedExpr::Mixed { .. }))
            ),
            "{input:?} should not merge"
        );
    }
}

#[test]
fn commas_and_brackets_mark_a_ladder_target_and_nothing_else_does() {
    for input in [
        "1.87 m to ft,in",
        "1.87 m to [ft, in]",
        "5.25 ft in ft,in",
        "5 ft 2 in in mi,ft",
        "1,000 m to ft,in",
        "7502 s -> [h, min, s]",
        "7502 s -> h,min",
    ] {
        let ParsedLine::Expression(ParsedExpr::UnOpPost { op, .. }) = parse(input) else {
            panic!("{input:?} should end in a conversion");
        };
        assert!(
            matches!(op, OpPost::Ladder(_)),
            "{input:?} should target a ladder"
        );
    }

    // A single unit, bracketed or not, is an ordinary conversion.
    for input in ["1 h to min", "5 m in [km]", "5 m + 3 in -> cm"] {
        let ParsedLine::Expression(ParsedExpr::UnOpPost { op, .. }) = parse(input) else {
            panic!("{input:?} should end in a conversion");
        };
        assert!(
            matches!(op, OpPost::Convert(_)),
            "{input:?} should target one dimension"
        );
    }
}

#[test]
fn parses_implicit_multiplication() {
    parse("5(2)(3)");
    parse("2pi");
}

#[test]
fn variable_multiplication_requires_number_first_and_no_space() {
    parse("5m2");
    assert!(parse_line("5 m2").is_err());
    assert!(parse_line("m2 5").is_err());
}

#[test]
fn rejects_garbage() {
    assert!(parse_line("5 +").is_err());
    assert!(parse_line("@#$").is_err());
}

#[test]
fn reports_unfinished_assignments_as_incomplete() {
    for input in ["x =", "x =   ", "letter =", "let result = "] {
        let error = parse_line(input).expect_err("assignment should be unfinished");
        assert_eq!(error.message(), "Incomplete expression");
    }

    let error = parse_line("5 =").expect_err("number is not a valid assignment name");
    assert_eq!(error.message(), "Unexpected '='");
}
