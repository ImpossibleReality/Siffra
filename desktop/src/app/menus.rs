//! The application menu bar and associated commands

use crate::editor::ShowCharacterPalette;
#[cfg(target_os = "macos")]
use crate::platform;
use crate::text::actions::{Copy, Cut, Paste, Redo, SelectAll, Undo};
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

pub fn register_actions(cx: &mut App) {
    #[cfg(target_os = "macos")]
    cx.on_action(|_: &About, _: &mut App| platform::macos::show_about_panel());
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

fn with_active_window(cx: &mut App, f: impl FnOnce(&mut Window, &mut App)) {
    let Some(window) = cx.active_window() else {
        return;
    };
    window.update(cx, |_, window, cx| f(window, cx)).ok();
}
