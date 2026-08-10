mod document;
mod store;

pub use document::{Document, now_millis};
pub use store::{DocumentStore, database_path};
