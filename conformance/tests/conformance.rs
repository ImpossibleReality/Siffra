//! Regression guard: no siffra unit may drift far from the `units(1)` reference.
//!
//! Runs the full sweep and fails on any *gross* disagreement (beyond
//! [`conformance::NEAR_TOL`]).
//!
//! Skips cleanly (rather than failing) when `units` is not installed

use conformance::{Status, cases, is_prefixed_case, reference_available, run_all, run_case};

#[test]
fn pound_mole_has_a_hand_checked_reference_factor() {
    let case = cases()
        .into_iter()
        .find(|case| case.symbol == "lbmol")
        .expect("lbmol catalog entry");
    assert!(matches!(run_case(&case).status, Status::Match { .. }));
}

#[test]
fn every_unit_agrees_with_reference() {
    if !reference_available() {
        eprintln!("skipping: `units(1)` not found on PATH");
        return;
    }

    let mut failures = Vec::new();
    for outcome in run_all() {
        match outcome.status {
            Status::Mismatch {
                siffra,
                reference,
                rel_err,
                matched,
                close: false,
                known: None,
            } => failures.push(format!(
                "  {} ({}): siffra {siffra} vs ref {reference} — rel err {rel_err:.3e} (via {matched})",
                outcome.symbol, outcome.name
            )),
            Status::NoReference { tried, .. } if !is_prefixed_case(&outcome.category) => failures
                .push(format!(
                    "  {} ({}): reference knows none of {}",
                    outcome.symbol,
                    outcome.name,
                    tried.join(", ")
                )),
            _ => {}
        }
    }

    assert!(
        failures.is_empty(),
        "{} unit(s) disagree with the reference beyond tolerance:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
