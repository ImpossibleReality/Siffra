//! The glyphs on the toolbar's buttons.
//!
//! The SVGs are embedded because GPUI's SVG element needs file-backed assets.
//! GPUI uses only their alpha as a mask and supplies the visible colour.

use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

// Helper macro for generating a GPUI `AssetSource` implementation
macro_rules! define_assets {
    (
        $(
            $(#[$kind_meta:meta])*
            $kind:ident {
                $(
                    $(#[$asset_meta:meta])*
                    $name:ident: $path:literal
                ),* $(,)?
            }
        )*
    ) => {
        $(
            $(#[$kind_meta])*
            pub mod $kind {
                $(
                    $(#[$asset_meta])*
                    pub const $name: &str = $path;
                )*
            }
        )*

        const ASSETS: &[(&str, &[u8])] = &[
            $(
                $(
                    (
                        $kind::$name,
                        include_bytes!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/assets/",
                            $path
                        )),
                    ),
                )*
            )*
        ];

        pub struct Assets;

        impl AssetSource for Assets {
            fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
                Ok(ASSETS
                    .iter()
                    .find(|(name, _)| *name == path)
                    .map(|(_, bytes)| Cow::Borrowed(*bytes)))
            }

            fn list(&self, path: &str) -> Result<Vec<SharedString>> {
                Ok(ASSETS
                    .iter()
                    .filter(|(name, _)| name.starts_with(path))
                    .map(|(name, _)| SharedString::from(*name))
                    .collect())
            }
        }
    };
}

define_assets! {
    icon {
        PLUS: "icons/plus.svg",
        SEARCH: "icons/search.svg",
        HELP: "icons/help.svg",
        PIN: "icons/pin.svg",
        CHEVRON: "icons/chevron.svg",
        CLOSE: "icons/close.svg",
    }
}
