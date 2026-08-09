//! Siffra — a notepad you can calculate in.

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
mod updater;
mod workspace;

fn main() {
    app::run();
}
