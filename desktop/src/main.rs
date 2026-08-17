//! Siffra -- a lightweight, fast, and accurate calculator with support for dimensional analysis

mod app;
mod calculation;
mod documents;
mod editor;
mod fuzzy;
mod help;
mod platform;
mod search;
mod syntax;
mod text;
mod ui;
mod updates;
mod workspace;

fn main() {
    app::run();
}
