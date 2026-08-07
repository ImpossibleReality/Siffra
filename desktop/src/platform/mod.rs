//! Native platform integrations that GPUI does not expose.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as current;
#[cfg(target_os = "macos")]
use macos as current;
#[cfg(target_os = "windows")]
use windows as current;

/// Holds the window above the others, or lets it back into the ordinary stack.
pub fn set_always_on_top(window: &gpui::Window, pinned: bool) {
    current::set_always_on_top(window, pinned);
}
