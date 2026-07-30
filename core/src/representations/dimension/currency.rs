//! Dynamic (currency) units.
//!
//! Currencies are the first units whose conversion factor is *not* a
//! compile-time constant: it is a live exchange rate resolved at evaluation
//! time. Rates come from the [Frankfurter](https://frankfurter.dev) API
//! (base currency EUR), are cached to disk, and refresh at most once an hour.
//! Everything degrades gracefully — a stale cache is preferred over a failed
//! fetch, and a missing rate makes the conversion fail (rather than panic or
//! invent a number).

use crate::representations::Float;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FRANKFURTER_URL: &str = "https://api.frankfurter.dev/v1/latest";
const CACHE_TTL_SECS: u64 = 3600;
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);
/// Minimum gap between background fetch attempts, so a failing API isn't hit on
/// every keystroke.
const RETRY_BACKOFF_SECS: u64 = 60;

/// The canonical currency: rates are expressed relative to it, and it is the
/// default collapse target for the currency axis.
const CANONICAL: &str = "EUR";

/// ISO 4217 codes Frankfurter publishes. Kept as a fixed set so that arbitrary
/// three-letter words aren't mistaken for currencies at parse time.
const KNOWN: &[&str] = &[
    "AUD", "BGN", "BRL", "CAD", "CHF", "CNY", "CZK", "DKK", "EUR", "GBP", "HKD", "HUF", "IDR",
    "ILS", "INR", "ISK", "JPY", "KRW", "MXN", "MYR", "NOK", "NZD", "PHP", "PLN", "RON", "SEK",
    "SGD", "THB", "TRY", "USD", "ZAR",
];

/// A currency unit, identified by its ISO code. Lives on the `Currency` axis;
/// its conversion factor is dynamic (see module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Currency {
    code: &'static str,
}

impl Currency {
    pub fn from_str(s: &str) -> Result<Self, ()> {
        KNOWN
            .iter()
            .find(|code| **code == s)
            .map(|code| Currency { code })
            .ok_or(())
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    /// The compact, unambiguous prefix symbol for this currency, when one is
    /// available. More symbols can be added here without changing callers.
    pub fn shorthand(&self) -> Option<&'static str> {
        match self.code {
            "USD" => Some("$"),
            _ => None,
        }
    }

    /// Every currency recognized by the parser, in ISO-code order.
    pub fn all() -> impl Iterator<Item = Currency> {
        KNOWN.iter().map(|code| Currency { code })
    }

    /// The English name of this currency (for search and display).
    pub fn name(&self) -> &'static str {
        currency_name(self.code)
    }

    /// The canonical currency (EUR).
    pub fn canonical() -> Self {
        Currency { code: CANONICAL }
    }

    /// The value of one unit of this currency in the canonical currency (EUR),
    /// i.e. its conversion factor. `None` when no rate is available.
    pub fn factor(&self) -> Option<Float> {
        // Frankfurter gives `code per 1 EUR`; the factor is its reciprocal.
        Some(&Float::from(1) / &rate(self.code)?)
    }
}

/// Maps an ISO 4217 code to its English name. Only the codes in [`KNOWN`] are
/// relevant; anything else falls back to the code itself.
fn currency_name(code: &str) -> &'static str {
    match code {
        "AUD" => "Australian dollar",
        "BGN" => "Bulgarian lev",
        "BRL" => "Brazilian real",
        "CAD" => "Canadian dollar",
        "CHF" => "Swiss franc",
        "CNY" => "Chinese yuan",
        "CZK" => "Czech koruna",
        "DKK" => "Danish krone",
        "EUR" => "Euro",
        "GBP" => "British pound",
        "HKD" => "Hong Kong dollar",
        "HUF" => "Hungarian forint",
        "IDR" => "Indonesian rupiah",
        "ILS" => "Israeli shekel",
        "INR" => "Indian rupee",
        "ISK" => "Icelandic króna",
        "JPY" => "Japanese yen",
        "KRW" => "South Korean won",
        "MXN" => "Mexican peso",
        "MYR" => "Malaysian ringgit",
        "NOK" => "Norwegian krone",
        "NZD" => "New Zealand dollar",
        "PHP" => "Philippine peso",
        "PLN" => "Polish złoty",
        "RON" => "Romanian leu",
        "SEK" => "Swedish krona",
        "SGD" => "Singapore dollar",
        "THB" => "Thai baht",
        "TRY" => "Turkish lira",
        "USD" => "US dollar",
        "ZAR" => "South African rand",
        _ => "",
    }
}

#[derive(Deserialize)]
struct FrankfurterResponse {
    rates: BTreeMap<String, f64>,
}

#[derive(Serialize, Deserialize)]
struct CachedRates {
    fetched_at: u64,
    /// `code -> units per 1 EUR` (EUR itself is 1.0).
    rates: BTreeMap<String, f64>,
}

struct State {
    table: Option<CachedRates>,
    /// When set, bypasses disk and network entirely (used by tests).
    override_rates: Option<BTreeMap<String, f64>>,
    /// Whether the one-time disk load has been attempted.
    loaded_disk: bool,
    /// Whether a background fetch is currently in flight.
    refreshing: bool,
    /// When the last background fetch was started (for retry backoff).
    last_attempt: u64,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(State {
            table: None,
            override_rates: None,
            loaded_disk: false,
            refreshing: false,
            last_attempt: 0,
        })
    })
}

/// Installs a fixed rate table (`code -> units per 1 EUR`) and disables all
/// disk/network access. For deterministic, offline tests.
pub fn install_static_rates(pairs: &[(&str, f64)]) {
    let mut rates: BTreeMap<String, f64> =
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    rates.insert(CANONICAL.to_string(), 1.0);
    state().lock().unwrap().override_rates = Some(rates);
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn is_fresh(table: &Option<CachedRates>) -> bool {
    table
        .as_ref()
        .is_some_and(|t| now().saturating_sub(t.fetched_at) < CACHE_TTL_SECS)
}

/// Resolves `code -> units per 1 EUR` **without ever blocking on the network**.
///
/// The in-memory table is seeded from disk once (a cheap local read); if it is
/// missing or stale, a background thread refreshes it, and the current
/// best-effort value is returned immediately — a stale rate, or `None` until
/// the first fetch lands (stale-while-revalidate).
fn rate(code: &str) -> Option<Float> {
    let mut state = state().lock().unwrap();

    if let Some(rates) = &state.override_rates {
        return rates.get(code).and_then(|r| to_float(*r));
    }

    if !state.loaded_disk {
        state.loaded_disk = true;
        state.table = load_disk();
    }

    let stale = !is_fresh(&state.table);
    let backoff_elapsed = now().saturating_sub(state.last_attempt) >= RETRY_BACKOFF_SECS;
    if stale && !state.refreshing && backoff_elapsed {
        state.last_attempt = now();
        state.refreshing = true;
        spawn_refresh();
    }

    state.table.as_ref()?.rates.get(code).and_then(|r| to_float(*r))
}

/// Fetches fresh rates off the evaluation thread and installs them. On failure
/// the existing (stale) table is left in place; backoff prevents rapid retries.
fn spawn_refresh() {
    std::thread::spawn(|| {
        let fetched = fetch();
        let mut state = state().lock().unwrap();
        state.refreshing = false;
        if let Some(rates) = fetched {
            let table = CachedRates {
                fetched_at: now(),
                rates,
            };
            save_disk(&table);
            state.table = Some(table);
        }
    });
}

fn to_float(rate: f64) -> Option<Float> {
    if !rate.is_finite() || rate <= 0.0 {
        return None;
    }
    Float::parse(&format!("{rate}")).ok()
}

fn fetch() -> Option<BTreeMap<String, f64>> {
    ensure_tls_provider();
    let client = reqwest::blocking::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .build()
        .ok()?;
    let body = client.get(FRANKFURTER_URL).send().ok()?.text().ok()?;
    let response: FrankfurterResponse = serde_json::from_str(&body).ok()?;
    let mut rates = response.rates;
    rates.insert(CANONICAL.to_string(), 1.0);
    Some(rates)
}

/// Installs the `ring` crypto provider as rustls' process default. Required
/// because reqwest is built with the `-no-provider` TLS feature, which keeps
/// the binary small. Runs once on the background fetch thread.
fn ensure_tls_provider() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn cache_path() -> Option<PathBuf> {
    let mut path = dirs::cache_dir()?;
    path.push("siffra");
    path.push("frankfurter.json");
    Some(path)
}

fn load_disk() -> Option<CachedRates> {
    let data = std::fs::read_to_string(cache_path()?).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_disk(table: &CachedRates) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(table) {
        let _ = std::fs::write(path, json);
    }
}
