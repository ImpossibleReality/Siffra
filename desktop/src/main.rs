//! Siffra — a notepad you can calculate in.
//!
//! The application draws an input column, a result column, and a rule tying each
//! line to its answer.

mod assets;
mod diagnostics;
mod documents;
mod editor;
mod engine;
mod fuzzy;
mod help;
mod highlight;
mod menus;
mod number_display;
mod platform;
mod relative_time;
mod search;
mod text_input;
mod theme;
mod toolbar;
mod ui;
mod updater;
mod workspace;

use assets::Assets;
use editor::layout::Fonts;
use editor::{
    DocumentEnd, DocumentStart, Down, Newline, SelectDown, SelectUp, ShowCharacterPalette, Up,
};
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions,
    point, px, size,
};
use menus::{CloseWindow, Hide, HideOthers, Minimize, Quit, ToggleFullScreen};
use text_input::{
    Backspace, Copy, Cut, Delete, DeleteToLineStart, DeleteWordLeft, DeleteWordRight, End, Home,
    Left, Paste, Redo, Right, SelectAll, SelectEnd, SelectHome, SelectLeft, SelectRight,
    SelectWordLeft, SelectWordRight, Undo, WordLeft, WordRight,
};
use workspace::{
    NameDocument, NewDocument, ToggleAlwaysOnTop, ToggleHelp, ToggleSearch, Workspace,
};

/// Preferred monospace families, in fallback order.
const MONO_FAMILIES: &[&str] = &[
    "Roboto Mono",
    "SF Mono",
    "Menlo",
    "Cascadia Mono",
    "Consolas",
    "DejaVu Sans Mono",
];

fn mono_family(cx: &App) -> &'static str {
    let available = cx.text_system().all_font_names();
    MONO_FAMILIES
        .iter()
        .copied()
        .find(|family| available.iter().any(|name| name == family))
        .unwrap_or("monospace")
}

/// The keys every text view answers to the same way, bound into the context of
/// whichever is asking. The calculation and the single-line fields share these
/// actions, so this is the one place the shape of text editing is written down.
///
/// What each view does *not* share it binds for itself: the calculation has
/// lines to move between, and a field has a line's worth of text to reach the
/// ends of with ⇧⌘←.
fn text_editing_bindings(context: &'static str) -> Vec<KeyBinding> {
    let context = Some(context);
    vec![
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("alt-backspace", DeleteWordLeft, context),
        KeyBinding::new("alt-delete", DeleteWordRight, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("alt-left", WordLeft, context),
        KeyBinding::new("alt-right", WordRight, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("shift-alt-left", SelectWordLeft, context),
        KeyBinding::new("shift-alt-right", SelectWordRight, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("cmd-left", Home, context),
        KeyBinding::new("cmd-right", End, context),
        KeyBinding::new("shift-home", SelectHome, context),
        KeyBinding::new("shift-end", SelectEnd, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new("cmd-c", Copy, context),
        KeyBinding::new("cmd-x", Cut, context),
        KeyBinding::new("cmd-v", Paste, context),
        KeyBinding::new("cmd-z", Undo, context),
        KeyBinding::new("cmd-shift-z", Redo, context),
    ]
}

fn key_bindings() -> Vec<KeyBinding> {
    let editor = Some("SiffraEditor");
    let mut bindings = text_editing_bindings("SiffraEditor");
    bindings.extend([
        // The calculation's own: a column of lines rather than one line.
        KeyBinding::new("cmd-backspace", DeleteToLineStart, editor),
        KeyBinding::new("up", Up, editor),
        KeyBinding::new("down", Down, editor),
        KeyBinding::new("shift-up", SelectUp, editor),
        KeyBinding::new("shift-down", SelectDown, editor),
        KeyBinding::new("cmd-up", DocumentStart, editor),
        KeyBinding::new("cmd-down", DocumentEnd, editor),
        KeyBinding::new("enter", Newline, editor),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, editor),
    ]);

    bindings.extend(text_editing_bindings("SiffraInput"));
    bindings.extend([
        // A field's own. ⌘⌫ is excluded inside the search modal, where it
        // deletes the highlighted calculation rather than the query — and a
        // binding is resolved before the modal's own key handling is reached.
        KeyBinding::new(
            "cmd-backspace",
            DeleteToLineStart,
            Some("SiffraInput && !SiffraSearch"),
        ),
        KeyBinding::new("shift-cmd-left", SelectHome, Some("SiffraInput")),
        KeyBinding::new("shift-cmd-right", SelectEnd, Some("SiffraInput")),
    ]);

    bindings.extend([
        // The window's own shortcuts, which reach the workspace from wherever the
        // keyboard happens to be — including a modal or the name field.
        KeyBinding::new("cmd-k", ToggleSearch, None),
        KeyBinding::new("cmd-/", ToggleSearch, None),
        // Written as the character rather than as ⇧ and the key it is typed
        // with: macOS reports the shifted character for a ⌘ chord, so ⌘⇧/ on a
        // US layout arrives as `cmd-?` — and on a layout where ? is typed some
        // other way, that is what arrives too. It is also what the Help menu
        // then advertises, as ⌘? rather than ⇧⌘/.
        KeyBinding::new("cmd-?", ToggleHelp, None),
        KeyBinding::new("cmd-n", NewDocument, None),
        KeyBinding::new("cmd-p", ToggleAlwaysOnTop, None),
        KeyBinding::new("cmd-s", NameDocument, None),
        // The app's own, which the menu bar reads back to label its items. Only
        // the first binding for an action is shown, so ⌘K is what Find advertises.
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("cmd-alt-h", HideOthers, None),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
    bindings
}

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        cx.bind_keys(key_bindings());
        menus::register_actions(cx);
        // After the bindings, since the items take their shortcuts from them.
        cx.set_menus(menus::app_menus());
        // And after the menus, since this takes back the ⌘? macOS would
        // otherwise have claimed for the Help menu's search field.
        platform::detach_native_help_menu();

        // Quitting is one of the moments a calculation is written, since what
        // was typed in the last fraction of a second has not been saved yet.
        // This covers every way out of the app, the Dock's Quit included.
        cx.on_app_quit(|cx: &mut App| {
            for window in cx.windows() {
                if let Some(window) = window.downcast::<Workspace>() {
                    window
                        .update(cx, |workspace, _, cx| workspace.flush_pending_save(cx))
                        .ok();
                }
            }
            async {}
        })
        .detach();

        // The window is the app: there is no document list or menu-bar item to
        // go back to, so closing the last one ends the session rather than
        // leaving a menu bar with nothing under it.
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let fonts = Fonts::new(mono_family(cx));
        let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);

        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Siffra".into()),
                        // The window draws its own bar, so the editor's
                        // background runs to the top edge.
                        appears_transparent: true,
                        // Left where the platform puts them, but lifted onto the
                        // toolbar's own centre line: the bar is a shade shorter
                        // than the title bar macOS would have drawn, and the
                        // buttons have to sit level with the name beside them.
                        traffic_light_position: Some(point(
                            px(theme::TRAFFIC_LIGHT_LEFT),
                            px(theme::traffic_light_top()),
                        )),
                    }),
                    window_min_size: Some(size(px(320.0), px(240.0))),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_rem_size(px(theme::REM));
                    cx.new(|cx| Workspace::new(fonts, window, cx))
                },
            )
            .expect("opening the window");

        window
            .update(cx, |workspace, window, cx| {
                window.focus(&workspace.editor_focus_handle(cx));
                cx.activate(true);
            })
            .ok();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{KeyContext, Keymap, Keystroke};

    /// The contexts a focused element sits under, outermost first.
    fn contexts(names: &[&str]) -> Vec<KeyContext> {
        names
            .iter()
            .map(|name| {
                let mut context = KeyContext::new_with_defaults();
                context.add(name.to_string());
                context
            })
            .collect()
    }

    /// What a keystroke resolves to for something focused under `stack`, or
    /// `None` when nothing claims it and the key is left to the views' own
    /// handlers.
    fn action_for(keystroke: &str, stack: &[&str]) -> Option<&'static str> {
        let keymap = Keymap::new(key_bindings());
        let keystroke = Keystroke::parse(keystroke).expect("a parseable keystroke");
        let (bindings, _) = keymap.bindings_for_input(&[keystroke], &contexts(stack));
        bindings.first().map(|binding| binding.action().name())
    }

    #[test]
    fn the_same_editing_keys_reach_both_the_calculation_and_a_field() {
        for context in ["SiffraEditor", "SiffraInput"] {
            assert_eq!(action_for("cmd-c", &[context]), Some("siffra::Copy"));
            assert_eq!(action_for("cmd-a", &[context]), Some("siffra::SelectAll"));
            assert_eq!(action_for("alt-left", &[context]), Some("siffra::WordLeft"));
        }
    }

    #[test]
    fn neither_view_answers_for_the_other() {
        // Lines belong to the calculation; ⇧⌘← to a field.
        assert_eq!(action_for("enter", &["SiffraInput"]), None);
        assert_eq!(action_for("shift-cmd-left", &["SiffraEditor"]), None);
        assert_eq!(
            action_for("enter", &["SiffraEditor"]),
            Some("siffra::Newline")
        );
        assert_eq!(
            action_for("shift-cmd-left", &["SiffraInput"]),
            Some("siffra::SelectHome")
        );
    }

    #[test]
    fn the_search_modal_keeps_its_own_delete() {
        // In a field ⌘⌫ clears back to the start, but in the search modal it
        // deletes the highlighted calculation — so nothing may claim it there,
        // or the modal's key handling would never be reached.
        assert_eq!(
            action_for("cmd-backspace", &["SiffraInput"]),
            Some("siffra::DeleteToLineStart")
        );
        assert_eq!(
            action_for("cmd-backspace", &["SiffraSearch", "SiffraInput"]),
            None
        );
    }

    #[test]
    fn the_windows_own_shortcuts_reach_through_a_field() {
        assert_eq!(
            action_for("cmd-k", &["SiffraInput"]),
            Some("siffra::ToggleSearch")
        );
        assert_eq!(
            action_for("cmd-?", &["SiffraInput"]),
            Some("siffra::ToggleHelp")
        );
        assert_eq!(
            action_for("cmd-p", &["SiffraInput"]),
            Some("siffra::ToggleAlwaysOnTop")
        );
        assert_eq!(
            action_for("cmd-w", &["SiffraInput"]),
            Some("siffra::CloseWindow")
        );
    }
}
