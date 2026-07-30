//! The menu bar, and the handful of commands that exist only because a menu bar
//! has to offer them.
//!
//! Nothing here does the work itself: every item dispatches an action that some
//! other part of the app already handles, so the menu is a second way in rather
//! than a second implementation. The shortcuts shown beside the items are read
//! back out of the keymap, which means a binding changed in `main` changes what
//! the menu advertises with it.
//!
//! A menu item is greyed out when its action is not reachable from wherever the
//! keyboard is — and a greyed-out item does not swallow its key equivalent, so
//! ⌘C in the name field still reaches the field even though the Edit menu also
//! claims it.

use crate::editor::ShowCharacterPalette;
use crate::platform;
use crate::text_input::{Copy, Cut, Paste, Redo, SelectAll, Undo};
use crate::workspace::{
    CheckForUpdates, InstallUpdate, NameDocument, NewDocument, ToggleAlwaysOnTop, ToggleHelp,
    ToggleSearch, Workspace,
};
use gpui::{App, Menu, MenuItem, SystemMenuType, Window, actions};

actions!(
    siffra,
    [
        About,
        CloseWindow,
        Hide,
        HideOthers,
        Minimize,
        Quit,
        ShowAll,
        ToggleFullScreen,
        Zoom
    ]
);

/// The bar as macOS draws it: the first menu is the application's own.
pub fn app_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Siffra".into(),
            items: vec![
                MenuItem::action("About Siffra", About),
                MenuItem::separator(),
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Hide Siffra", Hide),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit Siffra", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New Calculation", NewDocument),
                // Naming is what saving means here; see `workspace`.
                MenuItem::action("Name Calculation…", NameDocument),
                MenuItem::separator(),
                MenuItem::action("Find Calculation…", ToggleSearch),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", Cut),
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
                MenuItem::separator(),
                MenuItem::action("Select All", SelectAll),
                MenuItem::separator(),
                MenuItem::action("Emoji & Symbols", ShowCharacterPalette),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                // Neither reads as a checkbox: gpui's menus have no state to
                // show one with, so both are named as the switch they throw.
                MenuItem::action("Always on Top", ToggleAlwaysOnTop),
                MenuItem::separator(),
                MenuItem::action("Toggle Full Screen", ToggleFullScreen),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![
                MenuItem::action("Siffra Help", ToggleHelp),
                MenuItem::separator(),
                MenuItem::action("Check for Updates…", CheckForUpdates),
                MenuItem::action("Install Update…", InstallUpdate),
            ],
        },
    ]
}

/// Handles the commands that belong to the app rather than to the calculation:
/// the window's own chrome, and hiding or leaving the app altogether.
///
/// These are registered globally so they work no matter where the keyboard is —
/// including, for the menu's purposes, when nothing at all is focused.
pub fn register_actions(cx: &mut App) {
    cx.on_action(|_: &About, _: &mut App| platform::show_about_panel());
    cx.on_action(|_: &Hide, cx: &mut App| cx.hide());
    cx.on_action(|_: &HideOthers, cx: &mut App| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx: &mut App| cx.unhide_other_apps());

    cx.on_action(|_: &Minimize, cx: &mut App| {
        with_active_window(cx, |window, _| window.minimize_window());
    });
    cx.on_action(|_: &Zoom, cx: &mut App| {
        with_active_window(cx, |window, _| window.zoom_window());
    });
    cx.on_action(|_: &ToggleFullScreen, cx: &mut App| {
        with_active_window(cx, |window, _| window.toggle_fullscreen());
    });

    // Closing this way goes around the platform's own close, and so around the
    // flush hung off `on_window_should_close`; the write has to happen here.
    cx.on_action(|_: &CloseWindow, cx: &mut App| {
        let Some(handle) = cx
            .active_window()
            .and_then(|handle| handle.downcast::<Workspace>())
        else {
            return;
        };
        handle
            .update(cx, |workspace, window, cx| {
                workspace.flush_pending_save(cx);
                window.remove_window();
            })
            .ok();
    });

    cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
}

/// Runs `f` against the frontmost window, if the app has one.
fn with_active_window(cx: &mut App, f: impl FnOnce(&mut Window, &mut App)) {
    let Some(window) = cx.active_window() else {
        return;
    };
    window.update(cx, |_, window, cx| f(window, cx)).ok();
}
