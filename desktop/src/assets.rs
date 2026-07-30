//! The glyphs on the toolbar's buttons.
//!
//! The SVGs are embedded because GPUI's SVG element needs file-backed assets.
//! GPUI uses only their alpha as a mask and supplies the visible colour.

use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub mod icon {
    pub const PLUS: &str = "icons/plus.svg";
    pub const SEARCH: &str = "icons/search.svg";
    pub const HELP: &str = "icons/help.svg";
    pub const PIN: &str = "icons/pin.svg";
    pub const CHEVRON: &str = "icons/chevron.svg";
    pub const CLOSE: &str = "icons/close.svg";
}

const ICONS: &[(&str, &[u8])] = &[
    (icon::PLUS, include_bytes!("icons/plus.svg")),
    (icon::SEARCH, include_bytes!("icons/search.svg")),
    (icon::HELP, include_bytes!("icons/help.svg")),
    (icon::PIN, include_bytes!("icons/pin.svg")),
    (icon::CHEVRON, include_bytes!("icons/chevron.svg")),
    (icon::CLOSE, include_bytes!("icons/close.svg")),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}
