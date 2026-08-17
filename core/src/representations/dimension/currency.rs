//! Dynamic (currency) units.
//!
//! Currencies are resolved at evaluation time using the [Frankfurter](https://frankfurter.dev) API.
//! Exchange rates are cached to disk and refresh at most once an hour. On wasm, where neither the
//! network nor the disk is reachable from here, the host installs rates with
//! [`install_static_rates`] instead.

use crate::representations::Float;
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(target_arch = "wasm32"))]
const FRANKFURTER_URL: &str = "https://api.frankfurter.dev/v1/latest";
#[cfg(not(target_arch = "wasm32"))]
const CACHE_TTL_SECS: u64 = 3600;
#[cfg(not(target_arch = "wasm32"))]
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);
#[cfg(not(target_arch = "wasm32"))]
const RETRY_BACKOFF_SECS: u64 = 60;

const CANONICAL: &str = "EUR";

const KNOWN: &[&str] = &[
    "AUD", "BGN", "BRL", "CAD", "CHF", "CNY", "CZK", "DKK", "EUR", "GBP", "HKD", "HUF", "IDR",
    "ILS", "INR", "ISK", "JPY", "KRW", "MXN", "MYR", "NOK", "NZD", "PHP", "PLN", "RON", "SEK",
    "SGD", "THB", "TRY", "USD", "ZAR",
];

/// Currency identified by an ISO code; factors come from cached live rates.
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

    pub fn shorthand(&self) -> Option<&'static str> {
        match self.code {
            "USD" => Some("$"),
            _ => None,
        }
    }

    pub fn all() -> impl Iterator<Item = Currency> {
        KNOWN.iter().map(|code| Currency { code })
    }

    pub fn name(&self) -> &'static str {
        currency_name(self.code)
    }

    pub fn canonical() -> Self {
        Currency { code: CANONICAL }
    }

    pub fn factor(&self) -> Option<Float> {
        Some(&Float::from(1) / &rate(self.code)?)
    }
}

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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
struct FrankfurterResponse {
    rates: BTreeMap<String, f64>,
}

#[derive(Serialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Deserialize))]
struct CachedRates {
    fetched_at: u64,
    rates: BTreeMap<String, f64>,
}

#[derive(Default)]
struct State {
    table: Option<CachedRates>,
    override_rates: Option<BTreeMap<String, f64>>,
    #[cfg(not(target_arch = "wasm32"))]
    loaded_disk: bool,
    #[cfg(not(target_arch = "wasm32"))]
    refreshing: bool,
    #[cfg(not(target_arch = "wasm32"))]
    last_attempt: u64,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State::default()))
}

pub fn install_static_rates(pairs: &[(&str, f64)]) {
    let mut rates: BTreeMap<String, f64> = pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    rates.insert(CANONICAL.to_string(), 1.0);
    state().lock().unwrap().override_rates = Some(rates);
}

#[cfg(not(target_arch = "wasm32"))]
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(not(target_arch = "wasm32"))]
fn is_fresh(table: &Option<CachedRates>) -> bool {
    table
        .as_ref()
        .is_some_and(|t| now().saturating_sub(t.fetched_at) < CACHE_TTL_SECS)
}

fn rate(code: &str) -> Option<Float> {
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut state = state().lock().unwrap();

    if let Some(rates) = &state.override_rates {
        return rates.get(code).and_then(|r| to_float(*r));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
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
    }

    state
        .table
        .as_ref()?
        .rates
        .get(code)
        .and_then(|r| to_float(*r))
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
fn ensure_tls_provider() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn cache_path() -> Option<PathBuf> {
    let mut path = dirs::cache_dir()?;
    path.push("siffra");
    path.push("frankfurter.json");
    Some(path)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_disk() -> Option<CachedRates> {
    let data = std::fs::read_to_string(cache_path()?).ok()?;
    serde_json::from_str(&data).ok()
}

#[cfg(not(target_arch = "wasm32"))]
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
