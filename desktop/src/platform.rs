//! The few things the window needs that gpui does not hand out.

/// Holds the window above the others, or lets it back into the ordinary stack.
///
/// GPUI fixes a window's level when it opens, so pinning updates the underlying
/// `NSWindow` through the view GPUI exposes.
// The macro expansion references a cfg feature that this crate does not define.
#[allow(unexpected_cfgs)]
#[cfg(target_os = "macos")]
pub fn set_always_on_top(window: &gpui::Window, pinned: bool) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    const NS_NORMAL_WINDOW_LEVEL: isize = 0;
    const NS_FLOATING_WINDOW_LEVEL: isize = 3;

    // Use the raw-window-handle trait explicitly; gpui's method has different
    // semantics from the platform handle needed here.
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };

    // SAFETY: the handle gpui hands over is a live `NSView` for as long as the
    // window is open, and the window it belongs to outlives this call. Both
    // messages are sent on the main thread, which is the only place a gpui
    // window is updated from.
    unsafe {
        let view: *mut Object = handle.ns_view.as_ptr().cast();
        let native_window: *mut Object = msg_send![view, window];
        if native_window.is_null() {
            return;
        }
        let level = if pinned {
            NS_FLOATING_WINDOW_LEVEL
        } else {
            NS_NORMAL_WINDOW_LEVEL
        };
        let _: () = msg_send![native_window, setLevel: level];
    }
}

/// On non-macOS platforms the toggle remains available but has no window effect.
#[cfg(not(target_os = "macos"))]
pub fn set_always_on_top(_window: &gpui::Window, _pinned: bool) {}

/// Shows the About window macOS builds from the bundle's own description, so the
/// name and version it gives are whatever was shipped rather than a second copy
/// kept here.
// The macro expansion references a cfg feature that this crate does not define.
#[allow(unexpected_cfgs)]
#[cfg(target_os = "macos")]
pub fn show_about_panel() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: both messages go to the shared application on the main thread,
    // which is the only thread an action is dispatched on.
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let nil: *mut Object = std::ptr::null_mut();
        let _: () = msg_send![app, orderFrontStandardAboutPanel: nil];
    }
}

/// Elsewhere the About item is left out of the menu, so this is never reached.
#[cfg(not(target_os = "macos"))]
pub fn show_about_panel() {}
