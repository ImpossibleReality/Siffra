use cargo_packager_updater::{Config, Error, Update, check_update, semver::Version, url::Url};

pub const ENDPOINT: &str =
    "https://github.com/ImpossibleReality/Siffra/releases/latest/download/latest.json";

pub fn check() -> Result<Option<Update>, Error> {
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

pub fn install(update: &Update) -> Result<(), Error> {
    update.download_and_install()
}

pub fn summary(update: &Update) -> String {
    let notes = update
        .body
        .as_deref()
        .unwrap_or("No release notes were provided.")
        .split_whitespace()
        .take(12)
        .collect::<Vec<_>>()
        .join(" ");
    format!("Siffra {} is ready: {notes}", update.version)
}
