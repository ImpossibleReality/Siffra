use jiff::Zoned;
use jiff::civil::date;
use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::representations::{TimeZone, format_value};

fn zone() -> TimeZone {
    TimeZone::parse("America/Los_Angeles").expect("bundled tzdb")
}

fn state() -> SiffraState {
    let mut state = SiffraState::in_timezone(zone());
    state.set_now(fixed_now());
    state
}

fn fixed_now() -> Zoned {
    date(2026, 8, 8)
        .at(14, 32, 7, 0)
        .to_zoned(zone().inner().clone())
        .expect("a real wall clock time")
}

fn run(block: &str) -> Vec<String> {
    let mut state = state();
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

fn run_utc(block: &str) -> Vec<String> {
    let zone = TimeZone::utc();
    let now = date(2026, 8, 8)
        .at(14, 32, 7, 0)
        .to_zoned(zone.inner().clone())
        .expect("a real wall clock time");
    let mut state = SiffraState::in_timezone(zone.clone());
    state.set_now(now);

    block
        .lines()
        .map(|line| match evaluate_line(line, &mut state) {
            Ok(Some(value)) => format_value(&value, Some(&zone)).to_plain_string(),
            Ok(None) => "<none>".to_string(),
            Err(error) => format!("ERR: {}", error.message()),
        })
        .collect()
}

fn one_utc(line: &str) -> String {
    run_utc(line).pop().expect("one line of output")
}

#[test]
fn instant_literals_and_timezone_relabeling() {
    assert_eq!(one("2024-02-10"), "02-10-2024");
    assert_eq!(one("2024-02-10 12:00"), "02-10-2024 12:00 PM");
    assert_eq!(
        one("2024-02-10 12:00 America/Los_Angeles"),
        "02-10-2024 12:00 PM"
    );
    assert_eq!(
        one("2024-02-10 12:00 in Europe/Stockholm"),
        "02-10-2024 9:00 PM Europe/Stockholm"
    );
    assert_eq!(
        one("2024-02-10 in Asia/Kathmandu"),
        "02-10-2024 1:45 PM Asia/Kathmandu"
    );
    assert_eq!(one("now"), "08-08-2026 2:32:07 PM");
    assert_eq!(one("today"), "08-08-2026");
    assert_eq!(one("tomorrow"), "08-09-2026");
    assert_eq!(one("yesterday"), "08-07-2026");
}

#[test]
fn flexible_date_spellings_share_the_same_calendar_value() {
    for input in [
        "02-11-26",
        "2/11/26",
        "feb 11, 2026",
        "feb 11 2026",
        "February 11, 2026",
        "11 feb 2026",
    ] {
        assert_eq!(one(input), "02-11-2026", "input was {input:?}");
    }

    assert_eq!(one("2/11"), "0.181818181818181818181818181818");
    assert_eq!(one("2-11"), "-9");
}

#[test]
fn a_named_date_without_a_year_chooses_the_next_occurrence() {
    assert_eq!(one("aug 8"), "08-08-2026");
    assert_eq!(one("dec 1"), "12-01-2026");
    assert_eq!(one("feb 2"), "02-02-2027");
}

#[test]
fn flexible_clock_spellings_support_twelve_and_twenty_four_hour_time() {
    assert_eq!(one("5:00p"), "5:00 PM");
    assert_eq!(one("2am"), "2:00 AM");
    assert_eq!(one("5:30"), "5:30 AM");
    assert_eq!(one("17:20"), "5:20 PM");
    assert_eq!(one("12am"), "12:00 AM");
    assert_eq!(one("12pm"), "12:00 PM");
    assert!(one("13pm").starts_with("ERR:"));
}

#[test]
fn date_and_time_can_appear_in_either_order() {
    for input in [
        "5:00p on feb 2, 2026",
        "feb 2 2026 5:00p",
        "5:00p feb 2 2026",
    ] {
        assert_eq!(one(input), "02-02-2026 5:00 PM", "input was {input:?}");
    }
}

#[test]
fn datetime_zones_accept_utc_abbreviations_and_iana_names() {
    assert_eq!(one("feb 2 2026 5pm UTC"), "02-02-2026 5:00 PM UTC");
    assert_eq!(one("feb 2 2026 5pm PST"), "02-02-2026 5:00 PM PST");
    assert_eq!(
        one("feb 2 2026 5pm Europe/Stockholm"),
        "02-02-2026 5:00 PM Europe/Stockholm"
    );
}

#[test]
fn a_zone_can_immediately_follow_a_leading_or_bare_time() {
    assert_eq!(one("5pm UTC on feb 2 2026"), "02-02-2026 5:00 PM UTC");
    assert_eq!(one("5pm UTC feb 2 2026"), "02-02-2026 5:00 PM UTC");
    assert_eq!(one("5pm UTC today"), "08-08-2026 5:00 PM UTC");
    assert_eq!(one("5pm UTC"), "5:00 PM UTC");
    assert_eq!(one("5pm EST to PST"), "2:00 PM PST");
    assert_eq!(
        one("5pm America/New_York to America/Los_Angeles"),
        "2:00 PM"
    );
}

#[test]
fn a_time_followed_by_today_is_a_datetime() {
    assert_eq!(one("5pm today"), "08-08-2026 5:00 PM");
    assert_eq!(one("12:30:15am today"), "08-08-2026 12:30:15 AM");
}

#[test]
fn calendar_addition_clamps_and_keeps_the_wall_clock() {
    assert_eq!(one("2024-01-31 + 1 month"), "02-29-2024");
    assert_eq!(one("2024-03-09 12:00 + 1 day"), "03-10-2024 12:00 PM");
    assert_eq!(one("now + 3 weeks"), "08-29-2026 2:32:07 PM");
    assert_eq!(one("today - 90 days"), "05-10-2026");
}

#[test]
fn written_calendar_shifts_work_from_dates_times_and_now() {
    assert_eq!(one_utc("3 weeks after March 14, 2019"), "04-04-2019");
    assert_eq!(one_utc("28 days before March 12"), "02-12-2027");
    assert_eq!(one_utc("2 months 3 days after June 5"), "08-08-2027");
    assert_eq!(one_utc("2 weeks from now"), "08-22-2026 2:32:07 PM");
    assert_eq!(one_utc("90 min after 5:00 PM"), "6:30 PM");
}

#[test]
fn mixed_calendar_shifts_are_consistent_after_evaluation() {
    assert_eq!(
        run_utc(
            "x = 2 mo 2 d\n\
             x after today\n\
             today + x\n\
             2 mo 2 d after today\n\
             today + 2 mo 2 d"
        ),
        vec![
            "2 mo 2 d".to_string(),
            "10-10-2026".to_string(),
            "10-10-2026".to_string(),
            "10-10-2026".to_string(),
            "10-10-2026".to_string(),
        ]
    );
    assert_eq!(one_utc("2024-01-30 + 1 month 2 days"), "03-02-2024");
    assert_eq!(one_utc("2 months 2 days before today"), "06-06-2026");
    assert_eq!(
        run_utc(
            "span = 2024-02-10 to 2024-03-10 -> month,day\n\
             2024-01-30 + span"
        ),
        vec!["1 mo 0 d".to_string(), "02-29-2024".to_string()]
    );
}

#[test]
fn exact_durations_add_elapsed_seconds_across_a_dst_gap() {
    assert_eq!(one("2024-03-09 12:00 + 24 h"), "03-10-2024 1:00 PM");
    // Fractional calendar amounts fall back to exact seconds too.
    assert_eq!(one("2024-03-09 12:00 + 1.5 days"), "03-11-2024 1:00 AM");
}

#[test]
fn instants_reject_the_operations_that_have_no_meaning() {
    assert!(one("2024-02-10 + 2024-03-10").starts_with("ERR:"));
    assert!(one("1 day - 2024-03-10").starts_with("ERR:"));
    assert!(one("2024-02-10 in kg").starts_with("ERR:"));
}

#[test]
fn subtracting_instants_reads_every_calendar_axis() {
    assert_eq!(
        one("2024-02-10 to 2024-03-10"),
        "1 month / 29 days / 696 hours"
    );
    assert_eq!(
        one("2024-02-10 12:00 to 2024-03-10 12:00"),
        "1 month / 29 days / 695 hours"
    );
    assert_eq!(one("2024-02-10 to 2024-03-10 -> day"), "29 d");
    assert_eq!(one("2024-02-10 to 2024-03-10 -> hour"), "696 h");
    assert_eq!(
        one("2024-03-10 12:00 - 2024-02-10 12:00"),
        "1 month / 29 days / 695 hours"
    );
}

#[test]
fn a_reversed_range_reads_negative_on_every_axis() {
    assert_eq!(
        one("2024-03-10 to 2024-02-10"),
        "-1 month / -29 days / -696 hours"
    );
}

#[test]
fn a_range_between_the_same_date_is_zero() {
    assert_eq!(one("2024-02-10 to 2024-02-10"), "0 hours");
    assert_eq!(one("today to today -> day"), "0.0 d");
}

#[test]
fn conversion_selects_a_reading_without_reifying_it() {
    // 695/24 is not 29, so answering the second conversion from the first
    // would be wrong: both come from the anchors.
    assert_eq!(
        one("2024-02-10 12:00 to 2024-03-10 12:00 -> day -> hour"),
        "695 h"
    );
}

#[test]
fn plain_durations_keep_their_mean_length_ratios() {
    assert_eq!(one("12 h to d"), "0.5 d");
    assert_eq!(one("1 month to days"), "30.4375 d");
}

#[test]
fn combining_spans_degrades_to_calendar_readings() {
    assert_eq!(
        one("(2024-02-10 to 2024-03-10) + (2024-06-01 to 2024-07-01)"),
        "2 months / 59 days / 1,416 hours"
    );
    assert_eq!(
        one("0.5 * (2024-02-10 to 2024-03-10)"),
        "0.5 months / 14.5 days / 348 hours"
    );
    assert_eq!(
        one("(2024-02-10 to 2024-03-10) / 2"),
        "0.5 months / 14.5 days / 348 hours"
    );
}

#[test]
fn adding_a_plain_duration_drops_the_calendar_basis() {
    // There is no telling which endpoint the user meant to move, so the value
    // degrades to exact seconds rather than guessing.
    assert_eq!(one("(2024-02-10 to 2024-03-10) + 1 week"), "3,110,400 s");
}

#[test]
fn anchored_functions_need_a_single_range() {
    assert_eq!(
        one("midpoint(2024-02-10 to 2024-03-10)"),
        "02-24-2024 12:00 PM"
    );
    assert_eq!(one("start(2024-02-10 to 2024-03-10)"), "02-10-2024");
    assert_eq!(one("end(2024-02-10 to 2024-03-10)"), "03-10-2024");

    assert!(one("midpoint(1 month)").starts_with("ERR: Range Error"));
    assert!(
        one("midpoint((2024-02-10 to 2024-03-10) + (2024-06-01 to 2024-07-01))")
            .starts_with("ERR: Range Error")
    );
}

#[test]
fn clock_times_are_points_on_the_wall_clock() {
    assert_eq!(one("17:00"), "5:00 PM");
    assert_eq!(one("12:30:15"), "12:30:15 PM");
    assert!(one("25:00").starts_with("ERR: Time Error"));
}

#[test]
fn subtracting_clocks_measures_the_stretch_between_them() {
    assert_eq!(one("9:00 to 17:00"), "8 h");
    // A part-hour stretch reads off the clock face it came from.
    assert_eq!(one("17:00 - 9:15"), "7 h 45 min");
    assert_eq!(one("5:00p to 9:00p"), "4 h");
    // Clocks in different zones are the same kind of point, so this is the
    // stretch between two readings taken in two places.
    assert_eq!(one("9:00 UTC to 17:00"), "15 h");
}

#[test]
fn a_clock_shifted_by_a_duration_is_still_a_clock() {
    assert_eq!(one("2 h + 5:00p"), "7:00 PM");
    assert_eq!(one("5:00p + 2 h"), "7:00 PM");
    assert_eq!(one("5:00p - 30 min"), "4:30 PM");
    assert_eq!(one("12:30:15 + 1 h"), "1:30:15 PM");
    // Crossing midnight carries the reading around rather than past 24:00.
    assert_eq!(one("11:00p + 3 h"), "2:00 AM");
    assert_eq!(one("1:00a - 3 h"), "10:00 PM");
    // A whole day leaves the wall clock where it was.
    assert_eq!(one("5:00p + 1 day"), "5:00 PM");
}

#[test]
fn clocks_carry_a_zone_and_relabel_into_another() {
    assert_eq!(one("5pm UTC"), "5:00 PM UTC");
    assert_eq!(one("9:00 in Europe/Stockholm"), "6:00 PM Europe/Stockholm");
    assert_eq!(one("5pm EST to PST"), "2:00 PM PST");
    // The document's zone is the default, and needs no suffix of its own.
    assert_eq!(one("5pm America/Los_Angeles"), "5:00 PM");
}

#[test]
fn clocks_reject_the_operations_that_have_no_meaning() {
    assert!(one("9:00 + 10:00").starts_with("ERR: Unit Error"));
    assert!(one("5:00p in kg").starts_with("ERR: Unit Error"));
    assert_eq!(
        one("9:00 to 2024-02-10"),
        "ERR: Range Error",
        "a date and a clock are not two ends of one range"
    );
    // Twice a wall clock is not a time of day, so the affine unit decays.
    assert!(one("5:00p * 2").ends_with(" s"));
}

#[test]
fn fractional_totals_come_from_the_calendar_not_a_mean_year() {
    let years = one("1990-06-15 to today -> years");
    assert!(
        years.starts_with("36.1") && years.ends_with(" y"),
        "expected a fractional year total, got {years:?}"
    );
}

#[test]
fn a_span_is_still_the_exact_elapsed_seconds() {
    assert_eq!(one("2024-02-10 to 2024-03-10 -> s"), "2,505,600 s");
    assert_eq!(
        one("2024-02-10 12:00 to 2024-03-10 12:00 -> s"),
        "2,502,000 s"
    );
}

#[test]
fn dates_only_win_over_subtraction_when_they_are_written_as_one_token() {
    assert_eq!(one("2024 - 02 - 10"), "2,012");
    assert_eq!(one("2024-02-10 - 1 day"), "02-09-2024");
}

#[test]
fn multiplying_an_instant_decays_it_to_elapsed_seconds() {
    // Twice the epoch is not a date, so the affine unit becomes a linear one.
    assert_eq!(one("2024-02-10 * 2"), "3,415,104,000 s");
    assert_eq!(one("2024-02-10 / 2"), "853,776,000 s");
}

#[test]
fn timezone_names_do_not_shadow_unit_names() {
    assert_eq!(one("5 m in m/s"), "ERR: Unit Error");
    assert!(one("100 km / 2 h in mph").starts_with("31.0685596"));
}

#[test]
fn a_range_endpoint_can_be_any_temporal_expression() {
    assert_eq!(
        run("launch = 9:00\nlaunch to 17:30"),
        vec!["9:00 AM".to_string(), "8 h 30 min".to_string()]
    );
    assert_eq!(
        run("start = 2024-02-10\nend = 2024-03-10\nstart to end"),
        vec![
            "02-10-2024".to_string(),
            "03-10-2024".to_string(),
            "1 month / 29 days / 696 hours".to_string()
        ]
    );
    assert_eq!(one("9:00 to (8:00 + 3 h)"), "2 h");
    assert_eq!(one("today to tomorrow -> hours"), "24 h");
}

#[test]
fn to_reads_as_a_conversion_whenever_its_target_names_units() {
    assert_eq!(one("5 m in km"), "0.005 km");
    assert_eq!(one("9:00 to 17:00 in min"), "480 min");
    // A month name is also a unit name, and the date has to win.
    assert_eq!(one("dec 1 to dec 2 -> hours"), "24 h");
}

#[test]
fn a_name_that_is_neither_a_unit_nor_a_value_says_so() {
    for input in ["5 m in xyz", "today to xyz"] {
        assert_eq!(one(input), "ERR: Name Error", "input was {input:?}");
    }
    assert!(one("10 to 20").starts_with("ERR: Range Error"));
}

#[test]
fn variables_carry_instants_and_spans() {
    assert_eq!(
        run("launch = 2024-02-10\nlaunch + 1 month"),
        vec!["02-10-2024".to_string(), "03-10-2024".to_string()]
    );
    assert_eq!(
        run("trip = 2024-02-10 to 2024-03-10\ntrip -> day"),
        vec![
            "1 month / 29 days / 696 hours".to_string(),
            "29 d".to_string()
        ]
    );
}
