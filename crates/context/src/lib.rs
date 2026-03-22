//! Smart context windowing for grepit search results.
//!
//! Extracts surrounding context lines for each match, with support
//! for merging overlapping regions within the same file.

mod merge;
mod window;

pub use merge::merge_overlapping;
pub use window::{extract_context, ContextConfig, ContextualMatch};
