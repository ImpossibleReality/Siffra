//! Prints a conformance report: every siffra unit checked against `units(1)`.
//!
//! ```sh
//! cargo run -p conformance          # full report
//! cargo run -p conformance -- --fails-only
//! ```
//!
//! Exits non-zero if any unit disagrees with the reference by more than a small
//! convention-level tolerance, so it doubles as a CI gate.

use std::collections::BTreeMap;

use conformance::{EXACT_TOL, NEAR_TOL, Outcome, Status, run_all};

const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

fn main() {
    let fails_only = std::env::args().any(|a| a == "--fails-only");

    let Some(binary) = conformance::reference() else {
        eprintln!(
            "{RED}error:{RESET} GNU units not found on PATH.\n\
             Install it (macOS: `brew install gnu-units` -> `gunits`; \
             Debian/Ubuntu: `apt install units`), or point CONFORMANCE_UNITS=<binary> \
             at a GNU units executable."
        );
        std::process::exit(2);
    };

    println!(
        "{BOLD}siffra unit conformance{RESET} {DIM}vs GNU units (`{}`){RESET}\n\
         {DIM}values are 1 unit in canonical base units (m sec gram mol radian K bit ampere).\n\
         {GREEN}ok{RESET}{DIM}=exact  {YELLOW}≈ close{RESET}{DIM}=sub-tolerance  {YELLOW}≠ known{RESET}{DIM}=documented convention split  \
         {RED}MISMATCH{RESET}{DIM}=needs review{RESET}",
        binary
    );

    let outcomes = run_all();

    // Group by category, preserving first-seen order within the catalog.
    let mut order: Vec<String> = Vec::new();
    let mut by_category: BTreeMap<String, Vec<&Outcome>> = BTreeMap::new();
    for o in &outcomes {
        if !by_category.contains_key(&o.category) {
            order.push(o.category.clone());
        }
        by_category.entry(o.category.clone()).or_default().push(o);
    }

    let mut totals = Totals::default();

    for category in &order {
        let rows = &by_category[category];
        let shown: Vec<&&Outcome> = rows
            .iter()
            .filter(|o| !fails_only || is_fail(&o.status))
            .collect();
        if shown.is_empty() {
            for o in rows {
                totals.add(&o.status);
            }
            continue;
        }

        println!("\n{BOLD}{category}{RESET}");
        for o in rows {
            totals.add(&o.status);
            if fails_only && !is_fail(&o.status) {
                continue;
            }
            println!("  {}", format_row(o));
        }
    }

    print_summary(&totals);

    if totals.mismatch > 0 || totals.no_reference > 0 {
        std::process::exit(1);
    }
}

fn is_fail(status: &Status) -> bool {
    matches!(status, Status::NoReference { .. })
        || matches!(
            status,
            Status::Mismatch {
                close: false,
                known: None,
                ..
            }
        )
}

/// One aligned line: `symbol  name … status`.
fn format_row(o: &Outcome) -> String {
    let label = format!(
        "{:<8} {DIM}{:<22}{RESET}",
        truncate(&o.symbol, 8),
        truncate(&o.name, 22)
    );
    let detail = match &o.status {
        Status::Match {
            siffra, matched, ..
        } => format!(
            "{GREEN}ok{RESET}          {} {DIM}(= {}){RESET}",
            num(*siffra),
            matched
        ),
        Status::Mismatch {
            siffra,
            reference,
            rel_err,
            matched,
            close,
            known,
        } => {
            let (color, tag) = match (known.is_some(), *close) {
                (true, _) => (YELLOW, "≠ known   "),
                (false, true) => (YELLOW, "≈ close   "),
                (false, false) => (RED, "MISMATCH  "),
            };
            let note = known
                .as_deref()
                .map(|n| format!(", {n}"))
                .unwrap_or_default();
            format!(
                "{color}{tag}{RESET}siffra {} vs ref {} {DIM}(err {:.2e}, via {}{}){RESET}",
                num(*siffra),
                num(*reference),
                rel_err,
                matched,
                note
            )
        }
        Status::NoReference { siffra, .. } => format!(
            "{DIM}no ref      siffra {}{RESET}",
            if siffra.is_nan() {
                "—".to_string()
            } else {
                num(*siffra)
            }
        ),
        Status::Skipped { reason } => format!("{DIM}skipped     ({reason}){RESET}"),
    };
    format!("{label} {detail}")
}

fn print_summary(t: &Totals) {
    println!("\n{BOLD}Summary{RESET}");
    println!("  {GREEN}exact match{RESET}    {}", t.exact);
    println!("  {YELLOW}close (conv.){RESET}  {}", t.close);
    println!("  {YELLOW}known diff{RESET}     {}", t.known);
    println!("  {RED}mismatch{RESET}       {}", t.mismatch);
    println!("  {DIM}no reference{RESET}   {}", t.no_reference);
    println!("  {DIM}skipped{RESET}        {}", t.skipped);
    let checked = t.exact + t.close + t.known + t.mismatch;
    println!(
        "  {BOLD}{} of {} checked units agree or differ only by convention{RESET} (exact ≤ {:.0e}, close ≤ {:.0e})",
        t.exact + t.close + t.known,
        checked,
        EXACT_TOL,
        NEAR_TOL
    );
    if t.mismatch > 0 || t.no_reference > 0 {
        println!(
            "\n{RED}{} mismatch(es), {} unit(s) without a reference.{RESET}",
            t.mismatch, t.no_reference
        );
    } else {
        println!("\n{GREEN}No mismatches beyond convention tolerance.{RESET}");
    }
}

#[derive(Default)]
struct Totals {
    exact: usize,
    close: usize,
    known: usize,
    mismatch: usize,
    no_reference: usize,
    skipped: usize,
}

impl Totals {
    fn add(&mut self, status: &Status) {
        match status {
            Status::Match { .. } => self.exact += 1,
            Status::Mismatch { known: Some(_), .. } => self.known += 1,
            Status::Mismatch { close: true, .. } => self.close += 1,
            Status::Mismatch { close: false, .. } => self.mismatch += 1,
            Status::NoReference { .. } => self.no_reference += 1,
            Status::Skipped { .. } => self.skipped += 1,
        }
    }
}

fn num(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{v}");
    }
    let s = format!("{v:.6e}");
    s
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
