//! The few things the window needs that gpui does not hand out.

/// Runs `body` against the `NSWindow` behind a gpui window, if there is one.
// The macro expansion references a cfg feature that this crate does not define.
#[allow(unexpected_cfgs)]
#[cfg(target_os = "macos")]
fn with_native_window(window: &gpui::Window, body: impl FnOnce(*mut objc::runtime::Object)) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // Use the raw-window-handle trait explicitly; gpui's method has different
    // semantics from the platform handle needed here.
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };

    // SAFETY: the handle gpui hands over is a live `NSView` for as long as the
    // window is open, and the window it belongs to outlives this call. The
    // message is sent on the main thread, which is the only place a gpui window
    // is updated from.
    unsafe {
        let view: *mut Object = handle.ns_view.as_ptr().cast();
        let native_window: *mut Object = msg_send![view, window];
        if !native_window.is_null() {
            body(native_window);
        }
    }
}

/// Holds the window above the others, or lets it back into the ordinary stack.
///
/// GPUI fixes a window's level when it opens, so pinning updates the underlying
/// `NSWindow` through the view GPUI exposes.
// The macro expansion references a cfg feature that this crate does not define.
#[allow(unexpected_cfgs)]
#[cfg(target_os = "macos")]
pub fn set_always_on_top(window: &gpui::Window, pinned: bool) {
    use objc::{msg_send, sel, sel_impl};

    const NS_NORMAL_WINDOW_LEVEL: isize = 0;
    const NS_FLOATING_WINDOW_LEVEL: isize = 3;

    with_native_window(window, |native_window| {
        let level = if pinned {
            NS_FLOATING_WINDOW_LEVEL
        } else {
            NS_NORMAL_WINDOW_LEVEL
        };
        // SAFETY: sent to a live `NSWindow` on the main thread.
        unsafe {
            let _: () = msg_send![native_window, setLevel: level];
        }
    });
}

/// On non-macOS platforms the toggle remains available but has no window effect.
#[cfg(not(target_os = "macos"))]
pub fn set_always_on_top(_window: &gpui::Window, _pinned: bool) {}

/// Lets the window be dragged by its bar, or holds it still.
///
/// The bar is the window's drag handle, which the platform acts on before any of
/// this application's own handling is reached. Holding the window still is the
/// only way to let a press inside the bar mean something else — selecting text
/// in the name being the one thing it does mean.
// The macro expansion references a cfg feature that this crate does not define.
#[allow(unexpected_cfgs)]
#[cfg(target_os = "macos")]
pub fn set_window_movable(window: &gpui::Window, movable: bool) {
    use objc::runtime::{BOOL, NO, YES};
    use objc::{msg_send, sel, sel_impl};

    with_native_window(window, |native_window| {
        let movable: BOOL = if movable { YES } else { NO };
        // SAFETY: sent to a live `NSWindow` on the main thread.
        unsafe {
            let _: () = msg_send![native_window, setMovable: movable];
        }
    });
}

/// Elsewhere the window is dragged by whatever the platform decides.
#[cfg(not(target_os = "macos"))]
pub fn set_window_movable(_window: &gpui::Window, _movable: bool) {}

/// Takes the Help menu back from macOS.
///
/// AppKit adopts whichever menu is titled "Help" as its own: it inserts a search
/// field at the top and binds ⌘? to it, which reaches the menu bar before any of
/// this application's shortcuts and so would swallow the one that opens the help
/// panel. Naming a menu the app never shows leaves AppKit somewhere to put the
/// search field where nothing can be typed into it, and leaves ⌘? unclaimed.
// The macro expansion references a cfg feature that this crate does not define.
#[allow(unexpected_cfgs)]
#[cfg(target_os = "macos")]
pub fn detach_native_help_menu() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: both messages go to the shared application on the main thread,
    // which is where the app is set up. The menu is created owned and handed
    // over without a release: it has to outlive every other menu, and there is
    // exactly one of it for the life of the app.
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let unused_menu: *mut Object = msg_send![class!(NSMenu), new];
        let _: () = msg_send![app, setHelpMenu: unused_menu];
    }
}

/// Elsewhere the Help menu is the app's own to begin with.
#[cfg(not(target_os = "macos"))]
pub fn detach_native_help_menu() {}

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
