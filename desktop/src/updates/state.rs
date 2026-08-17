//! Current state of updates

use super::preferences::UpdatePreferences;
use super::release::{Release, current_version};
use gpui::SharedString;
use jiff::Timestamp;
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq)]
pub struct UpdateStatus {
    pub current_version: SharedString,
    pub next_version: Option<SharedString>,
    pub announced: bool,
    pub auto_update: bool,
}

impl UpdateStatus {
    pub fn available(&self) -> bool {
        self.next_version.is_some()
    }
}

#[derive(Default)]
pub struct UpdateState {
    release: Option<Arc<Release>>,
    preferences: UpdatePreferences,
}

impl UpdateState {
    pub fn load() -> Self {
        Self {
            release: None,
            preferences: UpdatePreferences::load(),
        }
    }

    pub fn status(&self) -> UpdateStatus {
        UpdateStatus {
            current_version: current_version(),
            next_version: self.release.as_ref().map(|release| release.version.clone()),
            announced: self.release.is_some() && !self.ignored(),
            auto_update: self.preferences.auto_update,
        }
    }

    /// Records this launch and reports when it is the first launch of a new version.
    pub fn record_launch(&mut self) -> Option<SharedString> {
        let current_version = current_version();
        let updated = self.preferences.record_launch(current_version.as_ref());
        self.preferences.save();
        updated.then_some(current_version)
    }

    pub fn release(&self) -> Option<Arc<Release>> {
        self.release.clone()
    }

    pub fn set_release(&mut self, release: Release) {
        self.release = Some(Arc::new(release));
    }

    pub fn ignored(&self) -> bool {
        self.release
            .as_ref()
            .is_some_and(|release| self.preferences.ignores(&release.version))
    }

    pub fn auto_update(&self) -> bool {
        self.preferences.auto_update
    }

    pub fn set_auto_update(&mut self, on: bool) {
        self.preferences.auto_update = on;
        self.preferences.save();
    }

    pub fn ignore_release(&mut self) {
        if let Some(release) = self.release.as_ref() {
            self.preferences.ignore(release.version.to_string());
        }
        self.preferences.auto_update = false;
        self.preferences.save();
    }

    pub fn set_last_check(&mut self, ts: Timestamp) {
        self.preferences.last_check = Some(ts);
        self.preferences.save();
    }

    pub fn last_check(&self) -> Option<Timestamp> {
        self.preferences.last_check
    }
}
