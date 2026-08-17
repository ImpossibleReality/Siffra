//! Markdown rendering for text that arrives as markdown rather than as
//! authored elements, such as release notes fetched from GitHub.

mod parse;
mod render;

pub use parse::{Block, parse};
pub use render::{MarkdownStyle, markdown};
