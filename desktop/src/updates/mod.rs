mod controller;
mod github;
mod modal;
mod preferences;
mod release;
mod state;
mod version_button;

pub use controller::{Updates, UpdatesEvent};
pub use modal::{UpdateModal, UpdateModalEvent};
pub use release::{AUTO_UPDATE_REFRESH_INTERVAL, CheckResult, check};
pub use state::{UpdateState, UpdateStatus};
pub use version_button::{CheckState, VersionButton, VersionButtonEvent};
