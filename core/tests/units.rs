//! End-to-end coverage for the unit/dimension system: every base axis and
//! derived-unit family, exercised through the real parser + evaluator so that
//! ratios, display shorthands, arithmetic, and axis collapse are all checked at
//! once.
//!
//! Conversion (`in`/`as`) applies after `*` and `/`, so a complete product or
//! quotient can be converted without extra parentheses.

use siffra::evaluation::{SiffraState, evaluate_line};
use siffra::representations::Value;

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

#[test]
fn data_bit_byte_and_prefixes() {
    assert_eq!(one("1 byte in bits"), "8 b");
    assert_eq!(one("16 bits in bytes"), "2 B");
    assert_eq!(one("1 kB in bytes"), "1,000 B");
    assert_eq!(one("1 MB in kB"), "1,000 kB");
    assert_eq!(one("1 KiB in bytes"), "1,024 B");
    assert_eq!(one("1 GiB in MiB"), "1,024 MiB");
    // Decimal (SI, x1000) and binary (IEC, x1024) prefixes share the axis.
    assert_eq!(one("1 GB in MiB"), "953.67431640625 MiB");
}

#[test]
fn data_rate_composes_with_time() {
    assert_eq!(one("100 Mbps * 8 s in MB"), "100 MB");
    assert_eq!(one("(1 GB) / (100 Mbps)"), "80 s");
}

#[test]
fn electrical_ohms_law_and_power() {
    assert_eq!(one("1000 mA in A"), "1 A");
    assert_eq!(one("2 A * 3 ohm in V"), "6 V");
    assert_eq!(one("12 V * 2 A in W"), "24 W");
    // `C` is Celsius, so use the long form when targeting coulombs.
    assert_eq!(one("2 A * 3 s in coulombs"), "6 C");
    assert_eq!(one("100 W * 1 h in kWh"), "0.1 kWh");
}

#[test]
fn volume_metric_and_us_customary() {
    assert_eq!(one("2 L in mL"), "2,000 mL");
    assert_eq!(one("1 gallon in L"), "3.785411784 L");
    assert_eq!(one("1 gallon in qt"), "4 qt");
    assert_eq!(one("1 qt in pt"), "2 pt");
    assert_eq!(one("1 cup in floz"), "8 floz");
    assert_eq!(one("1 tbsp in tsp"), "3 tsp");
    assert_eq!(one("1 L in cm^3"), "1,000 cm^3");
}

#[test]
fn area_units() {
    assert_eq!(one("1 hectare in m^2"), "10,000 m^2");
    assert_eq!(one("1 ha in are"), "100 are");
    assert_eq!(one("1 km^2 in hectare"), "100 ha");
}

#[test]
fn energy_units() {
    assert_eq!(one("1 kWh in J"), "3,600,000 J");
    assert_eq!(one("1 kcal in cal"), "1,000 cal");
    assert_eq!(one("1 cal in J"), "4.184 J");
    assert_eq!(one("1 kJ in J"), "1,000 J");
}

#[test]
fn power_units() {
    assert_eq!(one("1 kW in W"), "1,000 W");
    assert!(one("1 hp in W").starts_with("745.699871582270"));
    assert_eq!(one("2 kW * 30 min in kWh"), "1 kWh");
}

#[test]
fn pressure_units() {
    assert_eq!(one("1 bar in Pa"), "100,000 Pa");
    assert_eq!(one("1 atm in Pa"), "101,325 Pa");
    assert_eq!(one("1 bar in hPa"), "1,000 hPa");
    assert!(one("1 psi in Pa").starts_with("6,894.75729"));
}

#[test]
fn force_units() {
    assert_eq!(one("1 kN in N"), "1,000 N");
    assert_eq!(one("1 kg*m/s^2 in N"), "1 N");
}

#[test]
fn frequency_units() {
    assert_eq!(one("1 kHz in Hz"), "1,000 Hz");
    assert_eq!(one("1 GHz in MHz"), "1,000 MHz");
    assert_eq!(one("360 deg/min in rpm"), "1 rpm");
    assert!(one("60 rpm in rad/s").starts_with("6.283185307179"));
}

#[test]
fn speed_shorthands() {
    assert_eq!(one("100 kph in km/h"), "100 km/h");
    assert_eq!(one("60 mph in kph"), "96.56064 kph");
}

#[test]
fn dimensionless_scalars() {
    // Scalar units scale a number rather than merely labelling it.
    assert_eq!(one("5000 ppm as percent"), "0.5 %");
    assert_eq!(one("100 bp as percent"), "1 %");
    assert_eq!(one("1 % in bp"), "100 bp");
}

#[test]
fn extended_length_mass_time() {
    assert_eq!(one("1 au in km"), "149,597,870.7 km");
    assert_eq!(one("1 carat in g"), "0.2 g");
    assert_eq!(one("1 fortnight in days"), "14 d");
}

#[test]
fn extended_angle_units() {
    // A revolution is a full turn: 360 degrees, 400 gradians.
    assert_eq!(one("1 turn in deg"), "360 deg");
    assert_eq!(one("1 rev in deg"), "360 deg");
    assert_eq!(one("400 grad in rev"), "1 rev");
}

#[test]
fn rankine_absolute_temperature() {
    // 0 C = 273.15 K = 491.67 R; measured, like Kelvin, from absolute zero.
    assert_eq!(one("0 C in R"), "491.67 °R");
    assert_eq!(one("100 C in R"), "671.67 °R");
}

#[test]
fn arrow_converts_entire_line_without_parentheses() {
    // `in` binds tighter than `+`, so it converts only the last term; `->`
    // binds at the line level and converts the whole sum.
    assert_eq!(one("1 m + 3 in in cm"), "1.0762 m");
    assert_eq!(one("1 m + 3 in -> cm"), "107.62 cm");
    assert_eq!(one("(1 m + 3 in) in cm"), one("1 m + 3 in -> cm"));
    assert_eq!(one("1 m + 3 in ->cm"), "107.62 cm");
}

#[test]
fn arrow_converts_stored_value_on_assignment() {
    let mut state = SiffraState::new();
    assert_eq!(
        display_value(
            evaluate_line("x = 1 m + 3 in -> cm", &mut state)
                .unwrap()
                .unwrap()
        ),
        "107.62 cm"
    );
    assert_eq!(one_with("x", &mut state), "107.62 cm");
}

fn one_with(line: &str, state: &mut SiffraState) -> String {
    match evaluate_line(line, state) {
        Ok(Some(v)) => display_value(v),
        Ok(None) => "<none>".to_string(),
        Err(e) => format!("ERR: {}", e.message()),
    }
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
    assert_eq!(one("60 mph"), "60 mph");
    assert_eq!(one("100 km / 2 h"), "50 km/h");
    assert_eq!(one("2 Hz"), "2 Hz");
}

#[test]
fn torque_and_energy_do_not_collapse_into_each_other() {
    // Equal dimensionality does not erase deliberate display units.
    assert_eq!(one("5 N * 3 m"), "15 N*m");
    assert_eq!(one("5 J"), "5 J");
    assert!(
        one("12 N in J").starts_with("ERR"),
        "got {}",
        one("12 N in J")
    );
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

#[test]
fn si_prefixes_on_derived_units() {
    assert_eq!(one("1 uF in nF"), "1,000 nF");
    assert_eq!(one("2.2 uF + 800 nF"), "3 uF");
    assert_eq!(one("1 pF in farads"), "0.1E-11 F");
    assert_eq!(one("1 Mohm in kohm"), "1,000 kohm");
    assert_eq!(one("1 MOhm in ohm"), "1,000,000 ohm");
    assert_eq!(one("5 kV * 2 mA in W"), "10 W");
    assert_eq!(one("1 nH in henries"), "0.000000001 H");
    assert_eq!(one("1 uT in T"), "0.000001 T");
    assert_eq!(one("100 nF * 10 kohm in ms"), "1 ms");
    assert_eq!(one("1 GWh in kWh"), "1,000,000 kWh");
    assert_eq!(one("1 MeV in keV"), "1,000 keV");
    assert_eq!(one("1 kL in L"), "1,000 L");
    assert_eq!(one("1 kbar in bar"), "1,000 bar");
    assert_eq!(one("1 daN in N"), "10 N");
    assert_eq!(one("1 dPa in Pa"), "0.1 Pa");
}

#[test]
fn si_prefixes_on_base_units() {
    assert_eq!(one("1 Zm in Em"), "1,000 Em");
    assert_eq!(one("1 dam in dm"), "100 dm");
    assert_eq!(one("1 Mg in kg"), "1,000 kg");
    assert_eq!(one("1 Mt in t"), "1,000,000 t");
    assert_eq!(one("1 ks in s"), "1,000 s");
    assert_eq!(one("1 cmol in mmol"), "10 mmol");
    assert_eq!(one("1 krad in rad"), "1,000 rad");
    assert_eq!(one("1 daA in A"), "10 A");
    assert_eq!(one("1 Mg CO2 in mol"), one("1000 kg CO2 in mol"));
}

#[test]
fn si_prefixes_accept_spelled_out_names() {
    assert_eq!(one("1 nanofarad in pF"), "1,000 pF");
    assert_eq!(one("1 nanofarads in pF"), "1,000 pF");
    assert_eq!(one("1 megaohm in kohm"), "1,000 kohm");
    // A prefix and its unit must use the same symbol or word style.
    assert!(one("1 nanoF").starts_with("ERR"));
    assert!(one("1 nfarad").starts_with("ERR"));
}

#[test]
fn established_names_win_over_a_prefix_reading() {
    // Exact names win over possible prefix splits.
    assert_eq!(one("1 pc in ly"), "3.26156377716743358442662487194 ly");
    assert_eq!(one("5 min in s"), "300 s");
    assert_eq!(one("1 cal in J"), "4.184 J");
    assert_eq!(one("1 kt in knots"), "1 kn");
    assert_eq!(one("1 ct in g"), "0.2 g");
    assert_eq!(one("1 pt in floz"), "16 floz");
    assert_eq!(one("1 ha in are"), "100 are");
    assert_eq!(one("1 PS in W"), "735.49875 W");
    assert_eq!(one("1 nmi in m"), "1,852 m");
}

/// Established prefixed spellings resolve through the shared prefix table.
#[test]
fn removed_variants_still_parse_through_the_prefix_table() {
    let still_resolves = [
        "km",
        "cm",
        "mm",
        "um",
        "nm",
        "pm",
        "fm",
        "am",
        "dm",
        "dam",
        "hm",
        "Mm",
        "Gm",
        "Tm",
        "Pm",
        "Em",
        "kilometer",
        "centimeters",
        "micrometer",
        "nanometers",
        "mg",
        "ug",
        "ng",
        "pg",
        "dag",
        "hg",
        "milligram",
        "micrograms",
        "ms",
        "us",
        "ns",
        "ps",
        "fs",
        "msec",
        "usecs",
        "nsec",
        "psecs",
        "millisec",
        "milliseconds",
        "mmol",
        "umol",
        "nmol",
        "pmol",
        "kmol",
        "Mmol",
        "Gmol",
        "Tmol",
        "kgmol",
        "mgmol",
        "kilomole",
        "micromoles",
        "mrad",
        "milliradians",
        "mA",
        "uA",
        "nA",
        "kA",
        "milliamp",
        "milliamps",
        "kN",
        "kilonewtons",
        "kJ",
        "MJ",
        "kilojoules",
        "kWh",
        "MWh",
        "kilowatthours",
        "mW",
        "kW",
        "MW",
        "GW",
        "TW",
        "megawatts",
        "kPa",
        "hPa",
        "MPa",
        "hectopascals",
        "mbar",
        "millibars",
        "kHz",
        "MHz",
        "GHz",
        "THz",
        "kilohertz",
        "mL",
        "ml",
        "cL",
        "dL",
        "hL",
        "millilitres",
        "centiliters",
    ];
    for unit in still_resolves {
        let result = one(&format!("1 {unit}"));
        assert!(
            !result.starts_with("ERR"),
            "`{unit}` no longer resolves: {result}"
        );
    }

    assert_eq!(one("1 km in cm"), "100,000 cm");
    assert_eq!(one("1 msec in usec"), "1,000 us");
    assert_eq!(one("1 kgmol in mol"), "1,000 mol");
    assert_eq!(one("1 kWh in J"), "3,600,000 J");
    assert_eq!(one("1 mbar in Pa"), "100 Pa");
    assert_eq!(one("1 mL in cm^3"), "1 cm^3");
    assert_eq!(one("1 GHz in MHz"), "1,000 MHz");
    assert_eq!(one("1 milliamp in uA"), "1,000 uA");
}

#[test]
fn prefixed_units_expand_and_simplify() {
    // Prefix scaling is absorbed into the magnitude during expansion.
    assert_eq!(one("expand(5 kN)"), "5,000 kg*m/s^2");
    assert_eq!(one("simplify(5000 kg*m/s^2)"), "5,000 N");
}
