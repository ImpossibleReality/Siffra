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
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use siffra::representations::{
    AbsoluteTemperature, Amount, BaseDimension, DerivedUnit, Float, Quantity, Unit,
};

/// A unit paired with everything needed to check it: how to name it to the
/// reference tool, and how siffra defines it.
pub struct Case {
    pub category: String,
    pub name: String,
    pub symbol: String,
    pub names: Vec<String>,
    pub check: CheckKind,
}

/// How a given unit must be compared, since not every unit is a plain ratio.
pub enum CheckKind {
    Multiplicative { siffra_factor: f64, target: String },
    Affine(AbsoluteTemperature),
    Scalar { siffra_factor: f64, expected: f64 },
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

fn float_to_f64(value: &Float) -> f64 {
    value
        .to_string()
        .replace(',', "")
        .replace('E', "e")
        .parse()
        .unwrap_or(f64::NAN)
}

fn resolve_unit(names: &[String]) -> Option<Unit> {
    names.iter().find_map(|n| Unit::from_str(n).ok())
}

fn reference_names(symbol: &str, name: &str, aliases: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |candidate: String| {
        if !candidate.is_empty() && !out.contains(&candidate) {
            out.push(candidate);
        }
    };
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

fn prefixed_cases(catalog: &siffra::catalog::Catalog) -> Vec<Case> {
    let spelled_out = |name: &str| name.len() >= 3 && name.chars().all(|c| c.is_ascii_lowercase());

    let mut cases = Vec::new();
    for unit in catalog.units.iter().filter(|unit| unit.si_prefixes) {
        for prefix in catalog.prefixes.iter() {
            let symbol = format!("{}{}", prefix.symbol, unit.symbol);
            let name = format!("{}{}", prefix.name, unit.name);
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

pub fn is_prefixed_case(category: &str) -> bool {
    category.ends_with("(SI prefix)")
}

pub fn cases() -> Vec<Case> {
    let catalog = siffra::catalog::catalog();
    let mut prefixed = prefixed_cases(&catalog);
    let mut cases = Vec::new();

    for unit in catalog.units {
        if unit.symbol == "#" {
            continue;
        }
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
    if matches!(unit, Unit::Base(Quantity::Amount(Amount::PoundMole, _))) {
        let factor = unit.factor().map(|f| float_to_f64(&f)).unwrap_or(f64::NAN);
        return CheckKind::Scalar {
            siffra_factor: factor,
            expected: 453.59237,
        };
    }
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

pub fn reference_convert(source: &str, target: &str) -> Option<f64> {
    reference_eval(&format!("1 {source}"), target)
}

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

pub enum Status {
    Match {
        siffra: f64,
        reference: f64,
        rel_err: f64,
        matched: String,
    },
    Mismatch {
        siffra: f64,
        reference: f64,
        rel_err: f64,
        matched: String,
        close: bool,
        known: Option<String>,
    },
    NoReference {
        siffra: f64,
        tried: Vec<String>,
    },
    Skipped {
        reason: String,
    },
}

pub struct Outcome {
    pub category: String,
    pub name: String,
    pub symbol: String,
    pub status: Status,
}

pub const EXACT_TOL: f64 = 1e-9;
pub const NEAR_TOL: f64 = 1e-3;

const TEMP_ANCHORS: [f64; 3] = [37.0, 100.0, -40.0];

/// Documented cases where siffra and GNU units are different
pub fn known_difference(symbol: &str) -> Option<&'static str> {
    Some(match symbol {
        "Mach" => {
            "Mach 1 is temperature-dependent; siffra uses 340.29 m/s (15 °C), GNU units 331.46 m/s (0 °C)"
        }
        // GNU units' bare `year` is the *tropical* year (365.242198781 d, the
        // seasonal cycle); everything below it is derived from that. siffra uses
        // the Julian year (365.25 d exactly) — the IAU's definition, what `yr`
        // means in `ly` and in "3 years from now", and the only one of the two
        // that is a fixed number of days.
        "y" => {
            "siffra uses the Julian year (365.25 d); GNU units the tropical year (365.242198781 d)"
        }
        "mo" => {
            "one twelfth of siffra's Julian year (30.4375 d); GNU units divides the tropical year instead"
        }
        "decade" | "century" | "millennium" => {
            "a multiple of siffra's Julian year (365.25 d); GNU units multiplies the tropical year instead"
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

/// Compares an affine temperature at more than one anchor point
fn run_affine(temp: AbsoluteTemperature) -> Status {
    let func = temp_fn(temp);
    let source = |anchor: f64| format!("{func}({anchor})");

    let mut worst: Option<(f64, f64, f64)> = None;
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
pub fn run_all() -> Vec<Outcome> {
    let cases = cases();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
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
