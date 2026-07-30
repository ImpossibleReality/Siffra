//! Cross-checks siffra's unit definitions against a reference implementation of
//! unit conversion: **GNU units** (`units(1)`, installed by Homebrew as
//! `gunits`).
//!
//! # How the comparison works
//!
//! siffra stores every unit as a ratio (or, for a derived unit, a scalar times a
//! product of base units) into a fixed set of *canonical base units*:
//!
//! ```text
//! meter · second · gram · mole · radian · kelvin · bit · ampere
//! ```
//!
//! [`siffra::representations::Unit::factor`] returns the value of *one* of a unit
//! expressed in exactly those base units — e.g. `1 N` reduces to `1000` (grams,
//! not kilograms, so `1000 g·m/s²`). We never re-parse siffra's own output: we
//! read this factor straight off the enums, so the test exercises the unit
//! *definitions* directly.
//!
//! For the reference side we build the matching base-unit string and ask
//! `units` to convert `1 <name>` into it. Because the reference reduces the unit
//! independently, agreement means both agree on the physical magnitude.
//!
//! Three unit shapes need special handling and are documented at their call
//! sites: affine temperatures (`°C`, `°F` — an offset, not a ratio, checked via
//! GNU units' `tempC(x)` functions), pure dimensionless scalars (`%`, `ppm` —
//! checked against their exact definition), and currencies (a live exchange
//! rate — skipped).

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use siffra::representations::{
    AbsoluteTemperature, Amount, BaseDimension, DerivedUnit, Float, Quantity, Unit,
};

/// A unit paired with everything needed to check it: how to name it to the
/// reference tool, and how siffra defines it.
pub struct Case {
    /// Grouping label from the catalog (e.g. `Energy`, `Length`).
    pub category: String,
    /// Primary spelled-out name, for the report.
    pub name: String,
    /// Canonical short symbol, for the report.
    pub symbol: String,
    /// Every name to try against the reference tool, most specific first.
    pub names: Vec<String>,
    pub check: CheckKind,
}

/// How a given unit must be compared, since not every unit is a plain ratio.
pub enum CheckKind {
    /// A plain multiplicative unit: `siffra_factor` is its value in canonical
    /// base units, `target` is the matching reference base-unit string.
    Multiplicative { siffra_factor: f64, target: String },
    /// An affine temperature (offset scale): compared at several anchor points.
    Affine(AbsoluteTemperature),
    /// A dimensionless scalar (`%`, `ppm`, …). The reference tool cannot be
    /// trusted to parse these (`ppm` becomes pico·pico·meter), so it is checked
    /// against its exact definition instead.
    Scalar { siffra_factor: f64, expected: f64 },
    /// A currency: a live exchange rate, not a fixed conversion — skipped.
    Skip { reason: String },
}

/// The reference base unit for each physical axis, matching siffra's canonical
/// bases. Mass is spelled `gram` (siffra reduces mass to grams, not kilograms).
fn base_unit_name(axis: BaseDimension) -> Option<&'static str> {
    Some(match axis {
        BaseDimension::Length => "m",
        BaseDimension::Time => "sec",
        BaseDimension::Mass => "gram",
        BaseDimension::Amount => "mol",
        BaseDimension::Angle => "radian",
        BaseDimension::Temperature => "K",
        BaseDimension::Data => "bit",
        BaseDimension::ElectricCurrent => "ampere",
        // No stable reference unit; such a unit is skipped before we get here.
        BaseDimension::Currency => return None,
    })
}

/// The base-axis exponents contributed by *one* of this unit, summed per axis.
/// Rebuilt from public API (`Quantity::base_dimension`, `DerivedUnit::expansion`)
/// rather than [`siffra::representations::Dimensionality`], whose map is private.
fn axes(unit: &Unit) -> BTreeMap<BaseDimension, i64> {
    let mut map: BTreeMap<BaseDimension, i64> = BTreeMap::new();
    match unit {
        Unit::Base(quantity) => {
            *map.entry(quantity.base_dimension()).or_default() += 1;
        }
        Unit::Derived(derived) => {
            for (quantity, power) in derived.expansion() {
                *map.entry(quantity.base_dimension()).or_default() += power as i64;
            }
        }
        Unit::Absolute(_) => {
            *map.entry(BaseDimension::Temperature).or_default() += 1;
        }
        Unit::Currency(_) => {
            *map.entry(BaseDimension::Currency).or_default() += 1;
        }
        // An SI prefix is a pure scalar, so it contributes no axes of its own —
        // only the unit it is written on does. (Catalog units are unprefixed, so
        // this is here for completeness.)
        Unit::Prefixed(_, inner) => return axes(inner),
    }
    map.retain(|_, e| *e != 0);
    map
}

/// Builds the reference base-unit string for a set of axis exponents, e.g.
/// `{Mass:1, Length:1, Time:-2}` -> `"gram m / sec^2"`: a numerator, then a
/// single `/`, then a space-separated denominator.
fn target_string(axes: &BTreeMap<BaseDimension, i64>) -> Option<String> {
    let term = |name: &str, exp: i64| {
        if exp.abs() == 1 {
            name.to_string()
        } else {
            format!("{name}^{}", exp.abs())
        }
    };

    let mut numerator = Vec::new();
    let mut denominator = Vec::new();
    for (axis, exp) in axes {
        let name = base_unit_name(*axis)?;
        if *exp > 0 {
            numerator.push(term(name, *exp));
        } else {
            denominator.push(term(name, *exp));
        }
    }

    Some(match (numerator.is_empty(), denominator.is_empty()) {
        (true, true) => String::new(),
        (false, true) => numerator.join(" "),
        (true, false) => format!("/ {}", denominator.join(" ")),
        (false, false) => format!("{} / {}", numerator.join(" "), denominator.join(" ")),
    })
}

/// GNU units' absolute-temperature *function* for each scale: `tempC(20)` is
/// the point 20 °C, whereas the bare unit `degC` is only a 1 K interval.
fn temp_fn(temp: AbsoluteTemperature) -> &'static str {
    match temp {
        AbsoluteTemperature::Kelvin => "tempK",
        AbsoluteTemperature::Celsius => "tempC",
        AbsoluteTemperature::Fahrenheit => "tempF",
        AbsoluteTemperature::Rankine => "tempR",
    }
}

/// The exact value of each dimensionless scalar, as a fraction of one. These are
/// definitional (a percent *is* 1/100), so they are checked against the spec
/// rather than the reference tool, which cannot parse them reliably.
fn scalar_expected(derived: DerivedUnit) -> Option<f64> {
    use DerivedUnit::*;
    Some(match derived {
        Percent => 1e-2,
        Permille => 1e-3,
        Ppm => 1e-6,
        Ppb => 1e-9,
        Ppt => 1e-12,
        BasisPoint => 1e-4,
        _ => return None,
    })
}

/// Renders a siffra [`Float`] as an `f64`. The `Display` form is a decimal with
/// thousands separators, or `mantissaE±exp`; both parse once cleaned up.
fn float_to_f64(value: &Float) -> f64 {
    value
        .to_string()
        .replace(',', "")
        .replace('E', "e")
        .parse()
        .unwrap_or(f64::NAN)
}

/// Resolves a catalog unit to its siffra [`Unit`] by trying its names in order
/// (symbol, spelled-out name, then aliases) — the symbol alone does not always
/// round-trip (`°C` is spelled `celsius` to the resolver).
fn resolve_unit(names: &[String]) -> Option<Unit> {
    names.iter().find_map(|n| Unit::from_str(n).ok())
}

/// Candidate names to try against the reference tool, most specific first, each
/// also offered with spaces removed (`nautical mile` -> `nauticalmile`, which is
/// how `units` spells it). Deduplicated, order preserved.
fn reference_names(symbol: &str, name: &str, aliases: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |candidate: String| {
        if !candidate.is_empty() && !out.contains(&candidate) {
            out.push(candidate);
        }
    };
    // Spelled-out names first: they are far less likely to collide with an
    // unrelated reference unit than a one- or two-letter symbol.
    for raw in std::iter::once(name)
        .chain(std::iter::once(symbol))
        .chain(aliases.iter().map(String::as_str))
    {
        push(raw.to_string());
        if raw.contains(' ') {
            push(raw.replace(' ', ""));
        }
    }
    out
}

/// Cases for units written with an SI prefix, built from the catalog's own
/// prefix table and the units that declare they accept one.
///
/// The whole table is swept — every prefix on every prefixable unit — because
/// these spellings no longer have unit entries of their own to be checked as
/// plain cases. Where one still does (`kg`, `km`, `ha`), the plain case covers
/// it and the generated duplicate is dropped.
fn prefixed_cases(catalog: &siffra::catalog::Catalog) -> Vec<Case> {
    let spelled_out =
        |name: &str| name.len() >= 3 && name.chars().all(|c| c.is_ascii_lowercase());

    let mut cases = Vec::new();
    for unit in catalog.units.iter().filter(|unit| unit.si_prefixes) {
        for prefix in catalog.prefixes.iter() {
            let symbol = format!("{}{}", prefix.symbol, unit.symbol);
            let name = format!("{}{}", prefix.name, unit.name);
            // A spelled-out prefix only pairs with a spelled-out unit name.
            let aliases: Vec<String> = unit
                .aliases
                .iter()
                .filter(|alias| spelled_out(alias))
                .map(|alias| format!("{}{}", prefix.name, alias))
                .collect();

            let names = reference_names(&symbol, &name, &aliases);
            let Some(resolved) = resolve_unit(&names) else {
                continue;
            };
            if !matches!(resolved, Unit::Prefixed(..)) {
                continue;
            }

            cases.push(Case {
                category: format!("{} (SI prefix)", unit.category),
                name,
                symbol,
                check: build_check(&resolved),
                names,
            });
        }
    }
    cases
}

/// Whether a case is a prefixed spelling rather than a catalog unit. The
/// reference tool spells some of these differently or not at all, so a
/// [`Status::NoReference`] here is not a failure.
pub fn is_prefixed_case(category: &str) -> bool {
    category.ends_with("(SI prefix)")
}

/// Builds the full set of cases from siffra's own catalog, so any unit added to
/// the engine is picked up automatically.
pub fn cases() -> Vec<Case> {
    let catalog = siffra::catalog::catalog();
    // Built before the loop below consumes the catalog; appended after it so the
    // report leads with the plain units.
    let mut prefixed = prefixed_cases(&catalog);
    let mut cases = Vec::new();

    for unit in catalog.units {
        // The bare dimensionless number is not a convertible unit.
        if unit.symbol == "#" {
            continue;
        }
        // Currencies are live exchange rates, not fixed conversions.
        if unit.category == "Currency" {
            cases.push(Case {
                category: unit.category,
                name: unit.name,
                symbol: unit.symbol,
                names: Vec::new(),
                check: CheckKind::Skip {
                    reason: "live exchange rate".into(),
                },
            });
            continue;
        }

        let names = reference_names(&unit.symbol, &unit.name, &unit.aliases);

        let Some(resolved) = resolve_unit(&names) else {
            // Should not happen for a catalog unit, but never panic on it.
            cases.push(Case {
                category: unit.category,
                name: unit.name,
                symbol: unit.symbol,
                names,
                check: CheckKind::Skip {
                    reason: "siffra could not resolve its own unit name".into(),
                },
            });
            continue;
        };

        let check = build_check(&resolved);
        cases.push(Case {
            category: unit.category,
            name: unit.name,
            symbol: unit.symbol,
            names,
            check,
        });
    }

    cases.append(&mut prefixed);
    cases
}

fn build_check(unit: &Unit) -> CheckKind {
    // GNU units has no pound-mole entry. Keep an explicit definition check so
    // this high-risk factor cannot be reported as a successful "no ref" row.
    if matches!(
        unit,
        Unit::Base(Quantity::Amount(Amount::PoundMole, _))
    ) {
        let factor = unit.factor().map(|f| float_to_f64(&f)).unwrap_or(f64::NAN);
        return CheckKind::Scalar {
            siffra_factor: factor,
            expected: 453.59237,
        };
    }
    // GNU units does not recognize any of this unit's unambiguous aliases.
    // Check the exact definition directly instead of silently counting it.
    if matches!(unit, Unit::Derived(DerivedUnit::FootPound)) {
        let factor = unit.factor().map(|f| float_to_f64(&f)).unwrap_or(f64::NAN);
        return CheckKind::Scalar {
            siffra_factor: factor,
            expected: 1_355.817_948_331_400_4,
        };
    }
    if let Unit::Absolute(temp) = unit {
        return CheckKind::Affine(*temp);
    }
    if let Unit::Derived(derived) = unit {
        if let Some(expected) = scalar_expected(*derived) {
            let factor = unit.factor().map(|f| float_to_f64(&f)).unwrap_or(f64::NAN);
            return CheckKind::Scalar {
                siffra_factor: factor,
                expected,
            };
        }
    }

    let axes = axes(unit);
    let (Some(target), Some(factor)) = (target_string(&axes), unit.factor()) else {
        return CheckKind::Skip {
            reason: "no reference base unit for this dimension".into(),
        };
    };
    CheckKind::Multiplicative {
        siffra_factor: float_to_f64(&factor),
        target,
    }
}

/// The GNU units binary to invoke, detected once. Prefers `gunits`, then a
/// GNU `units` on `PATH`. Override with the `CONFORMANCE_UNITS` environment
/// variable. A non-GNU `units` is rejected.
pub fn reference() -> Option<&'static str> {
    static TOOL: OnceLock<Option<String>> = OnceLock::new();
    TOOL.get_or_init(detect_reference).as_deref()
}

fn detect_reference() -> Option<String> {
    if let Ok(binary) = std::env::var("CONFORMANCE_UNITS") {
        return is_gnu_units(&binary).then_some(binary);
    }
    ["gunits", "units"]
        .into_iter()
        .find(|b| is_gnu_units(b))
        .map(String::from)
}

/// Returns whether `binary` identifies itself as GNU units.
fn is_gnu_units(binary: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .output()
        .ok()
        .is_some_and(|o| {
            let mut banner = String::from_utf8_lossy(&o.stdout).into_owned();
            banner.push_str(&String::from_utf8_lossy(&o.stderr));
            banner.to_ascii_uppercase().contains("GNU")
        })
}

pub fn reference_available() -> bool {
    reference().is_some()
}

/// Converts `1 <source>` into `target` with GNU units, returning the numeric
/// factor. `None` when the tool does not recognise the unit or the two are not
/// conformable (GNU units exits non-zero in both cases).
pub fn reference_convert(source: &str, target: &str) -> Option<f64> {
    reference_eval(&format!("1 {source}"), target)
}

/// Runs `<gunits> -t -o '%.17g' <from> <to>` and parses a lone float out of
/// stdout, treating a non-zero exit as "no result".
fn reference_eval(from: &str, to: &str) -> Option<f64> {
    let binary = reference()?;
    let output = Command::new(binary)
        .args(["-t", "-o", "%.17g", from, to])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// The outcome of checking one unit.
pub enum Status {
    /// Matched the reference within tolerance. `rel_err` is the relative error.
    Match {
        siffra: f64,
        reference: f64,
        rel_err: f64,
        matched: String,
    },
    /// Disagreed with the reference. `close` marks a small (sub-`NEAR_TOL`)
    /// discrepancy, typically a definitional convention difference (which
    /// calorie, decimal vs. binary kilobyte) rather than a bug. `known` carries a
    /// note when the larger disagreement is an *understood* convention split
    /// where siffra is deliberately correct (see [`known_difference`]).
    Mismatch {
        siffra: f64,
        reference: f64,
        rel_err: f64,
        matched: String,
        close: bool,
        known: Option<String>,
    },
    /// The reference tool knows none of the unit's names.
    NoReference { siffra: f64, tried: Vec<String> },
    /// Deliberately not checked (currency, unresolved).
    Skipped { reason: String },
}

pub struct Outcome {
    pub category: String,
    pub name: String,
    pub symbol: String,
    pub status: Status,
}

/// Below this relative error a unit is considered an exact match.
pub const EXACT_TOL: f64 = 1e-9;
/// Below this, a mismatch is "close" — a convention difference, not a bug.
pub const NEAR_TOL: f64 = 1e-3;

/// Anchor temperatures (in each scale's own degrees) at which affine
/// temperatures are compared. Chosen away from `0 K`, where `units` errors.
const TEMP_ANCHORS: [f64; 3] = [37.0, 100.0, -40.0];

/// Documented cases where siffra and GNU units genuinely disagree because they
/// define a unit differently — and siffra's choice is the defensible one. Keyed
/// by symbol. These do not count as failures; they are annotated in the report
/// and excluded from the regression test, so only *new* drift breaks it.
pub fn known_difference(symbol: &str) -> Option<&'static str> {
    Some(match symbol {
        "Mach" => {
            "Mach 1 is temperature-dependent; siffra uses 340.29 m/s (15 °C), GNU units 331.46 m/s (0 °C)"
        }
        _ => return None,
    })
}

fn rel_err(a: f64, b: f64) -> f64 {
    if a == b {
        return 0.0;
    }
    let scale = a.abs().max(b.abs());
    if scale == 0.0 {
        0.0
    } else {
        (a - b).abs() / scale
    }
}

fn classify(siffra: f64, reference: f64, matched: String) -> Status {
    let err = rel_err(siffra, reference);
    if err <= EXACT_TOL {
        Status::Match {
            siffra,
            reference,
            rel_err: err,
            matched,
        }
    } else {
        Status::Mismatch {
            siffra,
            reference,
            rel_err: err,
            matched,
            close: err <= NEAR_TOL,
            known: None,
        }
    }
}

fn annotate(status: Status, symbol: &str) -> Status {
    if let Status::Mismatch {
        siffra,
        reference,
        rel_err,
        matched,
        close,
        known: None,
    } = status
    {
        if let Some(note) = known_difference(symbol) {
            return Status::Mismatch {
                siffra,
                reference,
                rel_err,
                matched,
                close,
                known: Some(note.to_string()),
            };
        }
        return Status::Mismatch {
            siffra,
            reference,
            rel_err,
            matched,
            close,
            known: None,
        };
    }
    status
}

pub fn run_case(case: &Case) -> Outcome {
    let status = match &case.check {
        CheckKind::Skip { reason } => Status::Skipped {
            reason: reason.clone(),
        },

        CheckKind::Scalar {
            siffra_factor,
            expected,
        } => classify(*siffra_factor, *expected, "definition".into()),

        CheckKind::Multiplicative {
            siffra_factor,
            target,
        } => {
            match case
                .names
                .iter()
                .find_map(|n| reference_convert(n, target).map(|r| (n.clone(), r)))
            {
                Some((matched, reference)) => classify(*siffra_factor, reference, matched),
                None => Status::NoReference {
                    siffra: *siffra_factor,
                    tried: case.names.clone(),
                },
            }
        }

        CheckKind::Affine(temp) => run_affine(*temp),
    };

    Outcome {
        category: case.category.clone(),
        name: case.name.clone(),
        status: annotate(status, &case.symbol),
        symbol: case.symbol.clone(),
    }
}

/// Compares an affine temperature at several anchor points. siffra converts to
/// absolute kelvin via [`AbsoluteTemperature::to_kelvin`]; GNU units converts the
/// same anchor with its `tempC(x)` *function* form (the bare `degC` unit is only
/// a 1 K interval). Reports the worst-case relative error across the anchors.
fn run_affine(temp: AbsoluteTemperature) -> Status {
    let func = temp_fn(temp);
    let source = |anchor: f64| format!("{func}({anchor})");

    let mut worst: Option<(f64, f64, f64)> = None; // (rel_err, siffra_k, ref_k)
    for anchor in TEMP_ANCHORS {
        let siffra_k = float_to_f64(&temp.to_kelvin(&Float::from(anchor)));
        let Some(ref_k) = reference_eval(&source(anchor), "K") else {
            continue;
        };
        let err = rel_err(siffra_k, ref_k);
        if worst.map_or(true, |(w, _, _)| err > w) {
            worst = Some((err, siffra_k, ref_k));
        }
    }

    match worst {
        Some((err, siffra_k, ref_k)) if err <= EXACT_TOL => Status::Match {
            siffra: siffra_k,
            reference: ref_k,
            rel_err: err,
            matched: func.to_string(),
        },
        Some((err, siffra_k, ref_k)) => Status::Mismatch {
            siffra: siffra_k,
            reference: ref_k,
            rel_err: err,
            matched: func.to_string(),
            close: err <= NEAR_TOL,
            known: None,
        },
        None => Status::NoReference {
            siffra: f64::NAN,
            tried: vec![func.to_string()],
        },
    }
}

/// Runs every case and returns the outcomes in catalog order.
///
/// Each case shells out to the reference tool, so the sweep is dominated by
/// process-spawn latency rather than by any work of our own. Splitting it across
/// a few threads is what keeps the full prefix table — every prefix on every
/// prefixable unit — affordable to check on every run.
pub fn run_all() -> Vec<Outcome> {
    let cases = cases();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    // Cases differ wildly in cost — some spawn the reference tool several times,
    // others not at all — so workers pull the next index rather than taking a
    // fixed slice.
    let next = AtomicUsize::new(0);

    let mut results: Vec<(usize, Outcome)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                let (next, cases) = (&next, &cases);
                scope.spawn(move || {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(case) = cases.get(index) else { break };
                        done.push((index, run_case(case)));
                    }
                    done
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("conformance worker panicked"))
            .collect()
    });

    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, outcome)| outcome).collect()
}
