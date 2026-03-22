//! Core search engine for grepit.
//!
//! Provides parallel regex matching over files with memory-mapped I/O
//! for maximum throughput.

mod binary;
mod engine;
mod filter;
mod matcher;

pub use binary::is_binary;
pub use engine::{SearchConfig, SearchEngine};
pub use filter::should_skip_path;
pub use matcher::RawMatch;

/// Re-export walker types that flow through the search pipeline.
pub use grepit_walker::{classify_file_type, FileEntry, FileType};
