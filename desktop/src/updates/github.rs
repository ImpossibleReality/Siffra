//! Reading a release's notes from GitHub.

use serde::Deserialize;
use std::time::Duration;

const LATEST_RELEASE: &str =
    "https://api.github.com/repos/ImpossibleReality/Siffra/releases/latest";
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct Release {
    body: Option<String>,
}

/// The latest release's notes fetched from GitHub
pub fn latest_release_notes() -> Option<String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        // GitHub rejects requests that do not name their caller.
        .user_agent(concat!("Siffra/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?
        .get(LATEST_RELEASE)
        .header("Accept", "application/vnd.github+json")
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .json::<Release>()
        .ok()?;

    response
        .body
        .map(|body| body.trim().to_string())
        .filter(|body| !body.is_empty())
        .map(|mut text| {
            // Changelog header -> What's New header
            text = text.replace("# Changelog", "# What's New");
            // Add github link to bottom
            text.push_str(
                "\n\nSee the [GitHub release](https://github.com/ImpossibleReality/Siffra/releases) for more details.",
            );

            text
        })
}
