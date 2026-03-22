//! Parallel search coordinator.
//!
//! This is the performance-critical hot path. It orchestrates:
//! - Parallel file reading via memory-mapped I/O
//! - Regex matching across all discovered files
//! - Binary file detection and skipping

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use rayon::prelude::*;
use memmap2::Mmap;

use grepit_walker::FileEntry;
use crate::binary::is_binary;
use crate::matcher::{RawMatch, find_matches};

/// Configuration for the search engine.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// The regex pattern to search for.
    pub pattern: String,
    /// Case-insensitive matching.
    pub ignore_case: bool,
    /// Match whole words only.
    pub word_boundary: bool,
    /// Treat pattern as a fixed string (not regex).
    pub fixed_string: bool,
    /// Maximum matches per file (None = unlimited).
    pub max_count_per_file: Option<usize>,
}

/// Statistics collected during the search.
#[derive(Debug, Clone, Default)]
pub struct SearchStats {
    pub files_searched: u64,
    pub files_skipped: u64,
    pub total_matches: u64,
}

/// The core search engine.
pub struct SearchEngine {
    config: SearchConfig,
    compiled_regex: regex::Regex,
}

impl SearchEngine {
    /// Create a new search engine with the given configuration.
    pub fn new(config: SearchConfig) -> Result<Self> {
        let pattern = Self::build_pattern(&config);
        let compiled_regex = regex::RegexBuilder::new(&pattern)
            .case_insensitive(config.ignore_case)
            .build()
            .map_err(|e| anyhow::anyhow!("Invalid regex pattern: {e}"))?;

        Ok(Self {
            config,
            compiled_regex,
        })
    }

    /// Build the final regex pattern from config options.
    fn build_pattern(config: &SearchConfig) -> String {
        let mut pattern = if config.fixed_string {
            regex::escape(&config.pattern)
        } else {
            config.pattern.clone()
        };

        if config.word_boundary {
            pattern = format!(r"\b{pattern}\b");
        }

        pattern
    }

    /// Search all given files in parallel. Returns matches and stats.
    pub fn search(&self, files: &[FileEntry]) -> (Vec<RawMatch>, SearchStats) {
        let files_searched = AtomicU64::new(0);
        let files_skipped = AtomicU64::new(0);

        let all_matches: Vec<RawMatch> = files
            .par_iter()
            .flat_map(|entry| {
                match self.search_file(&entry.path) {
                    Ok(matches) => {
                        files_searched.fetch_add(1, Ordering::Relaxed);
                        matches
                    }
                    Err(_) => {
                        files_skipped.fetch_add(1, Ordering::Relaxed);
                        Vec::new()
                    }
                }
            })
            .collect();

        let stats = SearchStats {
            files_searched: files_searched.load(Ordering::Relaxed),
            files_skipped: files_skipped.load(Ordering::Relaxed),
            total_matches: all_matches.len() as u64,
        };

        (all_matches, stats)
    }

    /// Search a single file using memory-mapped I/O.
    fn search_file(&self, path: &PathBuf) -> Result<Vec<RawMatch>> {
        let file = std::fs::File::open(path)?;
        let metadata = file.metadata()?;

        // Skip empty files
        if metadata.len() == 0 {
            return Ok(Vec::new());
        }

        // For small files, just read into memory (mmap overhead not worth it)
        let content = if metadata.len() < 32 * 1024 {
            std::fs::read(path)?
        } else {
            // SAFETY: We only read the file and don't hold the mapping
            // across any operations that might modify it.
            let mmap = unsafe { Mmap::map(&file)? };
            mmap.to_vec()
        };

        // Skip binary files
        if is_binary(&content) {
            return Ok(Vec::new());
        }

        Ok(find_matches(
            path,
            &content,
            &self.compiled_regex,
            self.config.max_count_per_file,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_pattern_fixed_string() {
        let config = SearchConfig {
            pattern: "foo.bar".to_string(),
            ignore_case: false,
            word_boundary: false,
            fixed_string: true,
            max_count_per_file: None,
        };
        let pattern = SearchEngine::build_pattern(&config);
        assert_eq!(pattern, r"foo\.bar");
    }

    #[test]
    fn test_build_pattern_word_boundary() {
        let config = SearchConfig {
            pattern: "test".to_string(),
            ignore_case: false,
            word_boundary: true,
            fixed_string: false,
            max_count_per_file: None,
        };
        let pattern = SearchEngine::build_pattern(&config);
        assert_eq!(pattern, r"\btest\b");
    }
}
