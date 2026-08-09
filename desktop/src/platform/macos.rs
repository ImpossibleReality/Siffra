//! macOS platform integrations.

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSFloatingWindowLevel, NSMenu, NSNormalWindowLevel, NSView, NSWindow,
};

fn with_native_window(window: &gpui::Window, body: impl FnOnce(&NSWindow)) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };

    // SAFETY: GPUI keeps this AppKit view alive for the window's lifetime.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    if let Some(native_window) = view.window() {
        body(&native_window);
    }
}

/// Keeps AppKit from adopting the application's Help menu.
pub(crate) fn detach_native_help_menu() {
    // AppKit inserts a search field into whichever menu is titled "Help" and
    // binds Command-? to it. Give that native behavior an unused menu so the
    // application's own help shortcut remains reachable.
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let unused_menu = NSMenu::new(mtm);
    app.setHelpMenu(Some(&unused_menu));
}

pub(super) fn set_always_on_top(window: &gpui::Window, pinned: bool) {
    with_native_window(window, |native_window| {
        let level = if pinned {
            NSFloatingWindowLevel
        } else {
            NSNormalWindowLevel
        };
        native_window.setLevel(level);
    });
}

/// Changes whether AppKit treats the custom title bar as a window drag handle.
pub(crate) fn set_window_movable(window: &gpui::Window, movable: bool) {
    with_native_window(window, |native_window| {
        native_window.setMovable(movable);
    });
}

/// Shows the standard panel populated from the application bundle.
pub(crate) fn show_about_panel() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    NSApplication::sharedApplication(mtm).orderFrontStandardAboutPanel(None);
}
