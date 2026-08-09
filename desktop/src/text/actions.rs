//! Editing commands shared by the calculation and compact text fields.

use gpui::actions;

// A view claims the actions it can honour in its own key context —
// `SiffraEditor` for the calculation and `SiffraInput` for a single-line field.
// Naming them independently of either view also lets the menu bar discover
// whether an action is available at the current focus.
actions!(
    siffra,
    [
        Backspace,
        Copy,
        Cut,
        Delete,
        DeleteToLineStart,
        DeleteWordLeft,
        DeleteWordRight,
        End,
        Home,
        Left,
        Paste,
        Redo,
        Right,
        SelectAll,
        SelectEnd,
        SelectHome,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        Undo,
        WordLeft,
        WordRight,
    ]
);
