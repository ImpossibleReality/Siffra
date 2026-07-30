//! Reusable application chrome and small interactive controls.
//!
//! Nothing here knows what a calculation, help entry, or file is. Features own
//! their data and behavior while these modules provide the visual and input
//! patterns shared between them.

pub mod code_block;
pub mod easing;
pub mod modal;
pub mod popover;
pub mod single_line_input;
pub mod toast;
pub mod tooltip;

/// The system interface font used by application controls.
pub const UI_FONT: &str = ".SystemUIFont";
