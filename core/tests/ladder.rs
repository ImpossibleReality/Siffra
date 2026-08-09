//! Mixed-unit ("ladder") display: one value written across several units.

use jiff::Zoned;
use jiff::civil::date;
use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::representations::{TimeZone, format_value};

fn zone() -> TimeZone {
    TimeZone::parse("America/Los_Angeles").expect("bundled tzdb")
}

// 2024-03-10 springs forward in this zone, which is what makes the anchored
// cases differ from the ratio ones.
fn fixed_now() -> Zoned {
    date(2026, 8, 8)
        .at(14, 32, 7, 0)
        .to_zoned(zone().inner().clone())
        .expect("a real wall clock time")
}

fn run(block: &str) -> Vec<String> {
    let mut state = SiffraState::in_timezone(zone());
    state.set_now(fixed_now());
    let mut out = Vec::new();
    for line in block.lines() {
        out.push(match evaluate_line(line, &mut state) {
            Ok(Some(value)) => format_value(&value, Some(&zone())).to_plain_string(),
            Ok(None) => "<none>".to_string(),
            Err(error) => format!("ERR: {}", error.message()),
        });
    }
    out
}

fn one(line: &str) -> String {
    run(line).pop().expect("one line of output")
}

fn description(line: &str) -> String {
    let mut state = SiffraState::in_timezone(zone());
    state.set_now(fixed_now());
    evaluate_line(line, &mut state)
        .expect_err("the line should fail")
        .description()
        .expect("a described failure")
}

#[test]
fn mixed_literals_read_back_the_way_they_were_written() {
    assert_eq!(one("5'2\""), "5 ft 2 in");
    assert_eq!(one("5 ft 2 in"), "5 ft 2 in");
    assert_eq!(one("2 h 5 min 2 s"), "2 h 5 min 2 s");
    assert_eq!(one("6 lb 3 oz"), "6 lb 3 oz");
}

#[test]
fn a_mixed_literal_is_an_ordinary_scalar_underneath() {
    assert_eq!(one("5'2\" in in"), "62 in");
    assert_eq!(one("2 h 5 min 2 s to s"), "7,502 s");
}

#[test]
fn only_a_shrinking_sequence_of_one_dimension_merges() {
    // Everything else keeps the reading it has today, which for a space
    // between two dimensioned numbers is no reading at all.
    for input in ["5 ft 2", "2 m 5 kg", "2 in 5 ft"] {
        assert!(
            one(input).starts_with("ERR:"),
            "{input:?} should keep today's meaning"
        );
    }
}

#[test]
fn a_ladder_target_splits_the_value_and_the_last_rung_keeps_the_fraction() {
    assert_eq!(one("1.87 m to ft,in"), "6 ft 1.622 in");
    assert_eq!(one("1.87 m to [ft, in]"), "6 ft 1.622 in");
    assert_eq!(one("100.5 in to ft,in"), "8 ft 4.5 in");
    assert_eq!(one("7502 s to [h, min, s]"), "2 h 5 min 2 s");
}

#[test]
fn every_requested_rung_is_shown_even_at_zero() {
    assert_eq!(one("0.5 d to h,min"), "12 h 0 min");
}

#[test]
fn a_single_unit_target_is_still_a_plain_conversion() {
    assert_eq!(one("1 h to min"), "60 min");
}

#[test]
fn the_convert_keyword_in_is_also_a_unit_and_both_readings_survive() {
    assert_eq!(one("5.25 ft in ft,in"), "5 ft 3 in");
    assert_eq!(one("5.6 ft in [ft, in]"), "5 ft 7.2 in");
    assert_eq!(one("5 ft 2 in in mi,ft"), "0 mi 5.167 ft");
    assert_eq!(one("3 in in cm"), "7.62 cm");
}

#[test]
fn commas_and_brackets_are_the_only_ladder_markers() {
    // A space-separated multi-unit target keeps its product meaning, which for
    // an unbracketed one is the parse error it already was.
    assert!(one("5 km to km m").starts_with("ERR:"));
    assert_eq!(one("100 J in [N * m]"), "100 N*m");
}

#[test]
fn a_grouped_number_does_not_collide_with_the_rung_separator() {
    assert_eq!(one("1,000 m to ft,in"), "3,280 ft 10.079 in");
}

#[test]
fn rungs_must_go_from_largest_to_smallest_and_measure_one_thing() {
    assert_eq!(
        description("5 m to in,ft"),
        "Ladder units must go from largest to smallest"
    );
    assert_eq!(
        description("5 m to [m, kg]"),
        "Ladder units must all measure the same thing"
    );
    assert_eq!(one("5 s to ft,in"), "ERR: Unit Error");
}

#[test]
fn a_ladder_survives_arithmetic_that_keeps_measuring_the_same_thing() {
    assert_eq!(one("5'2\" + 3 in"), "5 ft 5 in");
    // A ladder on either side is adopted; the left one wins when both have one.
    assert_eq!(one("3 in + 5'2\""), "5 ft 5 in");
    assert_eq!(one("2 * 5'2\""), "10 ft 4 in");
    assert_eq!(one("5'2\" / 2"), "2 ft 7 in");
}

#[test]
fn a_ladder_is_dropped_when_the_value_stops_being_what_it_described() {
    assert_eq!(one("5'2\" * 5'2\""), "3,844 in^2");
    assert_eq!(one("5'2\" to cm"), "157.48 cm");
    assert_eq!(one("(5'2\" + 3 in) to m"), "1.651 m");
}

#[test]
fn the_sign_is_written_once_in_front_of_the_rungs() {
    assert_eq!(one("-5 ft 2 in"), "-5 ft 2 in");
    assert_eq!(one("3 in - 5'2\""), "-4 ft 11 in");
}

#[test]
fn a_sign_in_front_of_a_zero_rung_is_bracketed_to_keep_it() {
    // `-0 ft 10 in` reads as ten positive inches to anyone who stops at the
    // first rung, so the sign takes a bracket to reach the whole value.
    assert_eq!(one("5'2\" - 6'0\""), "-(0 ft 10 in)");
    assert_eq!(one("-0.001 in to ft,in"), "-(0 ft 0.001 in)");
    // And what it prints is what it reads back as.
    assert_eq!(one("-(0 ft 10 in)"), "-(0 ft 10 in)");
    assert_eq!(one("-(0 ft 10 in) to in"), "-10 in");
}

#[test]
fn a_clock_difference_is_read_off_the_clock_face() {
    assert_eq!(one("17:00 - 9:15"), "7 h 45 min");
    // A whole number of hours says everything a minutes rung could.
    assert_eq!(one("9:00 to 17:00"), "8 h");
}

#[test]
fn clocks_written_with_seconds_are_answered_with_seconds() {
    // 45.25 min is the same number said less plainly, and the seconds were
    // asked for by writing them.
    assert_eq!(one("9:15:30 to 17:00:45"), "7 h 45 min 15 s");
    assert_eq!(one("9:00:00 to 17:00:00"), "8 h 0 min 0 s");
}

#[test]
fn a_last_rung_too_small_for_three_decimals_still_shows_itself() {
    // Rounding 0.0004 s to `0 s` is not a rounding of the value but a denial
    // of it, so the rung that holds the whole value keeps a digit.
    assert_eq!(one("0.0004 s to h,min,s"), "0 h 0 min 0.0004 s");
    assert_eq!(one("0.0005 s to h,min,s"), "0 h 0 min 0.001 s");
    // A crumb of binary precision under a real rung is not a quantity, and
    // rounds away as any other insignificant tail does.
    assert_eq!(one("1 ft to ft,in"), "1 ft 0 in");
    assert_eq!(one("0 s to h,min,s"), "0 h 0 min 0 s");
}

#[test]
fn only_units_that_add_up_and_hold_their_size_can_be_rungs() {
    // Celsius and Fahrenheit are points on a scale, not denominations: 100 °F
    // is not 55 °C plus 1 °F.
    assert_eq!(
        description("100 F to C,F"),
        "Ladder units must be amounts, not points on a scale"
    );
    assert_eq!(
        description("5 K to K,C"),
        "Ladder units must be amounts, not points on a scale"
    );
    // A currency's size is whatever the market says today, so the same ladder
    // would be valid one day and rejected the next.
    assert_eq!(
        description("1 USD to EUR,USD"),
        "Ladder units must be a fixed size, so currencies cannot be rungs"
    );
}

#[test]
fn a_duration_without_endpoints_splits_on_exact_ratios() {
    assert_eq!(one("695 h to d,h"), "28 d 23 h");
}

#[test]
fn an_anchored_span_splits_on_the_calendar_it_actually_crossed() {
    // 695 hours elapsed noon to noon, but 29 civil days: the ladder answers
    // from the endpoints, not from the scalar.
    assert_eq!(
        one("2024-02-10 12:00 to 2024-03-10 12:00 -> d,h"),
        "29 d 0 h"
    );
    assert_eq!(
        one("2024-02-10 12:00 to 2024-03-15 18:00 -> [mo, d, h]"),
        "1 mo 5 d 6 h"
    );
    // The single-unit reading is untouched by any of this.
    assert_eq!(one("2024-02-10 to 2024-03-10 -> d"), "29 d");
}

#[test]
fn a_calendar_rung_left_with_nothing_answers_zero() {
    // An earlier rung can land exactly on the end date. Jiff cannot total a
    // zero span in a calendar unit, and zero is the answer regardless.
    assert_eq!(one("2024-01-01 to 2024-02-01 -> mo,d"), "1 mo 0 d");
    assert_eq!(one("2024-02-10 to 2024-03-10 -> mo,d"), "1 mo 0 d");
    assert_eq!(one("2024-01-01 to 2025-01-01 -> y,mo"), "1 y 0 mo");
    assert_eq!(one("2024-01-01 to 2025-01-01 -> y,d"), "1 y 0 d");
    assert_eq!(one("2024-01-01 to 2024-01-08 -> w,d"), "1 w 0 d");
    // Including when there was nothing to divide in the first place, and when
    // the span runs backwards.
    assert_eq!(one("2024-03-31 to 2024-03-31 -> mo,d"), "0 mo 0 d");
    assert_eq!(one("2024-03-10 to 2024-02-10 -> mo,d"), "-1 mo 0 d");
}

#[test]
fn every_pair_of_calendar_rungs_answers_rather_than_panicking() {
    for rungs in [
        "y,mo", "y,w", "y,d", "y,h", "mo,d", "mo,h", "w,d", "w,h", "d,h", "d,min",
    ] {
        for span in [
            "2024-01-01 to 2025-01-01",
            "2024-02-10 12:00 to 2024-03-10 12:00",
            "2024-03-09 to 2024-03-11",
            "2024-03-31 to 2024-03-31",
            "2025-01-01 to 2024-01-01",
        ] {
            let line = format!("{span} -> {rungs}");
            let out = one(&line);
            assert!(!out.starts_with("ERR:"), "{line:?} answered {out:?}");
        }
    }
}

#[test]
fn the_larger_civil_units_are_counted_off_the_calendar_too() {
    // Each is a whole number of a unit the calendar knows, so it is counted
    // there rather than falling back to a mean-length ratio.
    assert_eq!(
        one("2024-03-01 to 2024-03-15 -> fortnight,d,h"),
        "1 fortnight 0 d 0 h"
    );
    assert_eq!(
        one("2024-03-01 to 2034-03-02 -> decade,y,d"),
        "1 decade 0 y 1 d"
    );
    assert_eq!(
        one("1900-01-01 to 2000-01-01 -> century,y,d"),
        "1 century 0 y 0 d"
    );
    assert_eq!(
        one("1000-01-01 to 2024-02-29 -> millennium,y,d"),
        "1 millennium 24 y 59 d"
    );
    // The fortnight that straddles the spring-forward is still fourteen civil
    // days, not 14 × 86,400 s.
    assert_eq!(
        one("2024-02-10 12:00 to 2024-03-10 12:00 -> fortnight,d,h"),
        "2 fortnight 1 d 0 h"
    );
}

#[test]
fn an_anchored_ladder_counts_the_repeated_hour_of_a_fall_back_once() {
    // 2024-11-03 falls back here, so those two civil days are 49 hours.
    assert_eq!(
        one("2024-11-02 12:00 to 2024-11-04 12:00 -> d,h"),
        "2 d 0 h"
    );
    assert_eq!(
        one("2024-11-02 12:00 to 2024-11-04 12:00 -> h,min"),
        "49 h 0 min"
    );
}

#[test]
fn a_span_between_two_zones_is_counted_on_the_calendar_it_started_from() {
    // Jiff will not count calendar units across two zones at all, and falling
    // back to a mean month here would answer 18.5 h for a five-hour remainder.
    assert_eq!(
        one("2024-01-01 00:00 UTC to 2024-02-01 00:00 America/New_York -> mo,d,h"),
        "1 mo 0 d 5 h"
    );
    assert_eq!(
        one("2024-02-01 00:00 America/New_York to 2024-01-01 00:00 UTC -> mo,d,h"),
        "-1 mo 0 d 5 h"
    );
    // 322 hours elapsed, but New York springs forward inside them.
    assert_eq!(
        one("2024-03-01 09:00 America/New_York to 2024-03-15 09:00 Asia/Tokyo -> d,h"),
        "13 d 11 h"
    );
}

#[test]
fn sub_day_rungs_are_exact_seconds_whatever_the_basis() {
    assert_eq!(
        one("2024-02-10 12:00 to 2024-03-10 12:00 -> [h, min]"),
        "695 h 0 min"
    );
    assert_eq!(
        one("((2024-02-10 to 2024-03-10) + (2024-06-01 to 2024-07-01)) -> h,min"),
        "1,416 h 0 min"
    );
}

#[test]
fn a_combined_range_has_no_hierarchy_to_split() {
    // Its readings are independent totals — 59 days and 1,416 hours disagree
    // about how the other divides up — so there is no honest answer.
    let line = "((2024-02-10 to 2024-03-10) + (2024-06-01 to 2024-07-01)) -> mo,d";
    assert_eq!(one(line), "ERR: Range Error");
    assert_eq!(
        description(line),
        "Cannot split a combined range into mo + d; convert to a single unit instead"
    );
}

#[test]
fn what_a_ladder_prints_reads_back_as_the_value_it_split() {
    // Around zero, the rung boundaries, the carry boundaries, and well past
    // where a ladder is comfortable.
    // The last rung carries three decimals, so a round trip is faithful to
    // half a millith of an inch and no further.
    let inches_of = |text: &str| {
        text.trim_end_matches(" in")
            .replace(',', "")
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("a plain inch reading, not {text:?}"))
    };
    for inches in [
        "0",
        "0.0005",
        "-0.0005",
        "11.999",
        "12",
        "12.001",
        "-12.001",
        "35.9995",
        "36",
        "63359.9",
        "63360",
        "-63360.5",
        "1000000.25",
    ] {
        let printed = one(&format!("{inches} in to mi,yd,ft,in"));
        let reparsed = inches_of(&one(&format!("({printed}) to in")));
        let expected = inches.parse::<f64>().expect("a test literal");
        // The bound is inclusive; f64 parsing lands either side of it.
        assert!(
            (reparsed - expected).abs() <= 0.0005 + 1e-9,
            "{inches} in printed as {printed:?} and read back as {reparsed}"
        );
    }
}

#[test]
fn a_ladder_travels_through_a_variable() {
    assert_eq!(
        run("height = 5'2\"\nheight + 1 in"),
        vec!["5 ft 2 in".to_string(), "5 ft 3 in".to_string()]
    );
}
