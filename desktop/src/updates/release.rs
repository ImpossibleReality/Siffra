//! Looking up the latest release and installing it.

use super::github::latest_release_notes;
use cargo_packager_updater::{Config, Error, Update, check_update, semver::Version, url::Url};
use gpui::SharedString;

pub const ENDPOINT: &str =
    "https://github.com/ImpossibleReality/Siffra/releases/latest/download/latest.json";

/// Forces the interface to behave as though a newer release were waiting for UI testing
pub const SIMULATE_UPDATE: bool = false;

/// How many days to cache latest release info for auto-updates
pub const AUTO_UPDATE_REFRESH_INTERVAL: i32 = 3;

const OFFLINE_NOTES: &str = include_str!("offline_notes.md");

pub fn current_version() -> SharedString {
    SharedString::new_static(env!("CARGO_PKG_VERSION"))
}

pub struct Release {
    pub version: SharedString,
    pub notes: SharedString,
    payload: Option<Update>,
}

impl Release {
    pub fn install(&self) -> Result<(), Error> {
        match self.payload.as_ref() {
            Some(update) => update.download_and_install(),
            None => Ok(()),
        }
    }

    fn from_update(update: Update) -> Self {
        Self {
            version: update.version.clone().into(),
            notes: update
                .body
                .clone()
                .unwrap_or_else(|| "No release notes were provided.".to_string())
                .into(),
            payload: Some(update),
        }
    }

    /// A stand-in release carrying the notes GitHub has published for the
    /// latest release, so what is drawn is the real thing.
    fn simulated() -> Self {
        Self {
            version: next_version().into(),
            notes: latest_release_notes()
                .map(SharedString::from)
                .unwrap_or(SharedString::new_static(OFFLINE_NOTES)),
            payload: None,
        }
    }
}

/// What a check found, or why it could not say.
pub type CheckResult = Result<Option<Release>, Error>;

pub fn check() -> CheckResult {
    let found = fetch()?.map(Release::from_update);
    if found.is_none() && SIMULATE_UPDATE {
        return Ok(Some(Release::simulated()));
    }
    Ok(found)
}

fn fetch() -> Result<Option<Update>, Error> {
    let Some(public_key) = option_env!("SIFFRA_UPDATER_PUBLIC_KEY").filter(|key| !key.is_empty())
    else {
        return Ok(None);
    };

    let config = Config {
        endpoints: vec![Url::parse(ENDPOINT).expect("the built-in update endpoint is valid")],
        pubkey: public_key.into(),
        ..Default::default()
    };
    let version = Version::parse(env!("CARGO_PKG_VERSION"))
        .expect("the package version is valid semantic versioning");

    check_update(version, config)
}

/// The version a simulated release claims to be: this build's, one patch on.
fn next_version() -> String {
    let mut version = Version::parse(env!("CARGO_PKG_VERSION"))
        .expect("the package version is valid semantic versioning");
    version.patch += 1;
    version.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_simulated_release_is_newer_than_this_build() {
        let simulated = Version::parse(&next_version()).expect("a version");
        let current = Version::parse(&current_version()).expect("a version");
        assert!(simulated > current);
    }

    #[test]
    fn a_simulated_release_cannot_install_anything() {
        assert!(Release::simulated().install().is_ok());
    }
}
