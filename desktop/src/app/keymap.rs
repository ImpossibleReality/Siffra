use crate::app::menus::{CloseWindow, Hide, HideOthers, Minimize, Quit, ToggleFullScreen};
use crate::editor::{
    DocumentEnd, DocumentStart, Down, Newline, SelectDown, SelectUp, ShowCharacterPalette, Up,
};
use crate::text::actions::{
    Backspace, Copy, Cut, Delete, DeleteToLineStart, DeleteWordLeft, DeleteWordRight, End, Home,
    Left, Paste, Redo, Right, SelectAll, SelectEnd, SelectHome, SelectLeft, SelectRight,
    SelectWordLeft, SelectWordRight, Undo, WordLeft, WordRight,
};
use crate::workspace::{NameDocument, NewDocument, ToggleAlwaysOnTop, ToggleHelp, ToggleSearch};
use gpui::KeyBinding;

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

pub fn bindings() -> Vec<KeyBinding> {
    let editor = Some("SiffraEditor");
    let mut bindings = text_editing_bindings("SiffraEditor");
    bindings.extend([
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
        KeyBinding::new(
            "cmd-backspace",
            DeleteToLineStart,
            Some("SiffraInput && !SiffraSearch"),
        ),
        KeyBinding::new("shift-cmd-left", SelectHome, Some("SiffraInput")),
        KeyBinding::new("shift-cmd-right", SelectEnd, Some("SiffraInput")),
    ]);
    bindings.extend([
        KeyBinding::new("cmd-k", ToggleSearch, None),
        KeyBinding::new("cmd-/", ToggleSearch, None),
        KeyBinding::new("cmd-?", ToggleHelp, None),
        KeyBinding::new("cmd-n", NewDocument, None),
        KeyBinding::new("cmd-p", ToggleAlwaysOnTop, None),
        KeyBinding::new("cmd-s", NameDocument, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("cmd-alt-h", HideOthers, None),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
    bindings
}
