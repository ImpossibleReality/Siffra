//! Save preferences for auto-updating + ignoring new versions

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DATA_DIRECTORY: &str = "dev.impossiblereality.siffra";
const PREFERENCES_FILE: &str = "updates.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdatePreferences {
    pub auto_update: bool,
    pub ignored_version: Option<String>,
    pub last_check: Option<Timestamp>,
    pub last_launched_version: Option<String>,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            auto_update: true,
            ignored_version: None,
            last_check: None,
            last_launched_version: None,
        }
    }
}

impl UpdatePreferences {
    pub fn record_launch(&mut self, version: &str) -> bool {
        let updated = self
            .last_launched_version
            .as_deref()
            .is_some_and(|previous| previous != version);
        self.last_launched_version = Some(version.to_string());
        updated
    }

    pub fn ignores(&self, version: &str) -> bool {
        self.ignored_version.as_deref() == Some(version)
    }

    pub fn ignore(&mut self, version: impl Into<String>) {
        self.ignored_version = Some(version.into());
    }

    pub fn load() -> Self {
        preferences_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|stored| serde_json::from_str(&stored).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = preferences_path() else {
            return;
        };
        let Ok(encoded) = serde_json::to_string_pretty(self) else {
            return;
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, encoded).ok();
    }
}

fn preferences_path() -> Option<PathBuf> {
    Some(
        dirs::data_dir()?
            .join(DATA_DIRECTORY)
            .join(PREFERENCES_FILE),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_are_automatic_until_a_release_is_dismissed_for_good() {
        let mut preferences = UpdatePreferences::default();
        assert!(preferences.auto_update);
        assert!(!preferences.ignores("0.3.2"));

        preferences.ignore("0.3.2");
        preferences.auto_update = false;
        assert!(preferences.ignores("0.3.2"));
        assert!(!preferences.ignores("0.3.3"));
    }

    #[test]
    fn a_new_version_is_reported_once_after_restart() {
        let mut preferences = UpdatePreferences::default();

        assert!(!preferences.record_launch("0.3.0"));
        assert!(!preferences.record_launch("0.3.0"));
        assert!(preferences.record_launch("0.3.1"));
        assert!(!preferences.record_launch("0.3.1"));
    }
}
