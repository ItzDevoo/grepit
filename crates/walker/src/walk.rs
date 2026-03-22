//! Parallel directory walker wrapping the `ignore` crate.

use std::path::{Path, PathBuf};
use crossbeam_channel::{Sender, Receiver, bounded};
use ignore::WalkBuilder;
use crate::filetype::{FileType, classify_file_type};

/// Configuration for the directory walker.
#[derive(Debug, Clone)]
pub struct WalkerConfig {
    /// Root paths to search.
    pub paths: Vec<PathBuf>,
    /// Number of parallel threads (0 = auto).
    pub threads: usize,
    /// Whether to respect .gitignore files.
    pub respect_gitignore: bool,
    /// Whether to search hidden files/directories.
    pub search_hidden: bool,
    /// Maximum directory depth (0 = unlimited).
    pub max_depth: Option<usize>,
    /// Maximum file size in bytes (0 = unlimited).
    pub max_filesize: Option<u64>,
    /// Glob patterns to include.
    pub globs: Vec<String>,
    /// File types to include (empty = all).
    pub include_types: Vec<String>,
    /// File types to exclude.
    pub exclude_types: Vec<String>,
}

impl Default for WalkerConfig {
    fn default() -> Self {
        Self {
            paths: vec![PathBuf::from(".")],
            threads: 0,
            respect_gitignore: true,
            search_hidden: false,
            max_depth: None,
            max_filesize: None,
            globs: Vec::new(),
            include_types: Vec::new(),
            exclude_types: Vec::new(),
        }
    }
}

/// A discovered file entry ready for searching.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub file_type: FileType,
}

/// Parallel directory walker that discovers files for searching.
pub struct Walker {
    config: WalkerConfig,
}

impl Walker {
    pub fn new(config: WalkerConfig) -> Self {
        Self { config }
    }

    /// Walk the configured paths and return a vector of file entries.
    /// This is the simple synchronous API.
    pub fn collect_files(&self) -> Vec<FileEntry> {
        let mut entries = Vec::new();
        let builder = self.build_walker();

        let walker = builder.build_parallel();
        let (tx, rx): (Sender<FileEntry>, Receiver<FileEntry>) = bounded(4096);

        // Spawn the walker in a scoped thread
        std::thread::scope(|s| {
            s.spawn(|| {
                walker.run(|| {
                    let tx = tx.clone();
                    let max_filesize = self.config.max_filesize;
                    let include_types = self.config.include_types.clone();
                    let exclude_types = self.config.exclude_types.clone();

                    Box::new(move |entry| {
                        let entry = match entry {
                            Ok(e) => e,
                            Err(_) => return ignore::WalkState::Continue,
                        };

                        // Skip directories
                        let ft = entry.file_type();
                        if ft.is_none_or(|t| !t.is_file()) {
                            return ignore::WalkState::Continue;
                        }

                        let path = entry.path().to_path_buf();

                        // Check file size limit
                        if let Some(max_size) = max_filesize {
                            if let Ok(meta) = entry.metadata() {
                                if meta.len() > max_size {
                                    return ignore::WalkState::Continue;
                                }
                            }
                        }

                        let file_type = classify_file_type(&path);

                        // Apply type filters
                        if !include_types.is_empty()
                            && !include_types.iter().any(|t| t == file_type.name())
                        {
                            return ignore::WalkState::Continue;
                        }
                        if exclude_types.iter().any(|t| t == file_type.name()) {
                            return ignore::WalkState::Continue;
                        }

                        let _ = tx.send(FileEntry { path, file_type });
                        ignore::WalkState::Continue
                    })
                });
                drop(tx);
            });

            for entry in rx {
                entries.push(entry);
            }
        });

        entries
    }

    /// Build the underlying ignore walker with our configuration.
    fn build_walker(&self) -> WalkBuilder {
        let first = self.config.paths.first().map(|p| p.as_path()).unwrap_or(Path::new("."));
        let mut builder = WalkBuilder::new(first);

        // Add additional paths
        for path in self.config.paths.iter().skip(1) {
            builder.add(path);
        }

        let threads = if self.config.threads == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        } else {
            self.config.threads
        };

        builder
            .threads(threads)
            .hidden(!self.config.search_hidden)
            .git_ignore(self.config.respect_gitignore)
            .git_global(self.config.respect_gitignore)
            .git_exclude(self.config.respect_gitignore);

        if let Some(depth) = self.config.max_depth {
            builder.max_depth(Some(depth));
        }

        // Add glob overrides
        if !self.config.globs.is_empty() {
            let mut overrides = ignore::overrides::OverrideBuilder::new(first);
            for glob in &self.config.globs {
                overrides.add(glob).ok();
            }
            if let Ok(built) = overrides.build() {
                builder.overrides(built);
            }
        }

        builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_walker_config_default() {
        let config = WalkerConfig::default();
        assert!(config.respect_gitignore);
        assert!(!config.search_hidden);
        assert_eq!(config.threads, 0);
    }
}
