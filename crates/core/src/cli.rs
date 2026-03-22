//! CLI argument definitions using clap derive.

use clap::Parser;

/// grep4ai — The fastest grep tool built for AI agents.
///
/// Searches for PATTERN in files, returning structured, ranked results
/// optimized for AI agent consumption.
#[derive(Parser, Debug)]
#[command(name = "grep4ai", version, about, long_about = None)]
#[command(
    after_help = "EXAMPLES:\n  grep4ai \"fn main\"                    Search for 'fn main' in current directory\n  grep4ai -f json --pretty Config src/  Search with pretty JSON output\n  grep4ai --token-budget 2000 TODO      Search with token budget\n  grep4ai -t rust \"impl.*Display\"       Search only Rust files\n  grep4ai -F --dedup \"import React\"     Find & deduplicate exact string"
)]
pub struct Args {
    /// The regex pattern to search for.
    #[arg(required = true)]
    pub pattern: String,

    /// Files or directories to search (default: current directory).
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    // ── Output Options ──────────────────────────────────────────────
    /// Output format: json (default), jsonl, compact, human.
    #[arg(short = 'f', long = "format", default_value = "json")]
    pub format: String,

    /// Pretty-print JSON output.
    #[arg(long)]
    pub pretty: bool,

    /// Omit statistics from output.
    #[arg(long = "no-stats")]
    pub no_stats: bool,

    // ── Search Options ──────────────────────────────────────────────
    /// Case-insensitive search.
    #[arg(short = 'i', long = "ignore-case")]
    pub ignore_case: bool,

    /// Match whole words only.
    #[arg(short = 'w', long = "word")]
    pub word: bool,

    /// Treat pattern as a literal string, not regex.
    #[arg(short = 'F', long = "fixed-strings")]
    pub fixed_strings: bool,

    /// Only search files of this type (e.g., rust, python, js).
    #[arg(short = 't', long = "type")]
    pub file_type: Vec<String>,

    /// Exclude files of this type.
    #[arg(short = 'T', long = "type-not")]
    pub type_not: Vec<String>,

    /// Include files matching this glob pattern.
    #[arg(short = 'g', long = "glob")]
    pub glob: Vec<String>,

    /// Don't respect .gitignore files.
    #[arg(long = "no-ignore")]
    pub no_ignore: bool,

    /// Search hidden files and directories.
    #[arg(long)]
    pub hidden: bool,

    /// Maximum matches per file.
    #[arg(short = 'm', long = "max-count")]
    pub max_count: Option<usize>,

    /// Skip files larger than this size (e.g., 1M, 500K).
    #[arg(long = "max-filesize")]
    pub max_filesize: Option<String>,

    /// Maximum directory traversal depth.
    #[arg(long = "max-depth")]
    pub max_depth: Option<usize>,

    // ── Context Options ─────────────────────────────────────────────
    /// Lines of context around each match.
    #[arg(short = 'C', long = "context")]
    pub context: Option<usize>,

    /// Lines of context after each match.
    #[arg(short = 'A', long = "after")]
    pub after_context: Option<usize>,

    /// Lines of context before each match.
    #[arg(short = 'B', long = "before")]
    pub before_context: Option<usize>,

    /// Merge overlapping context regions in the same file.
    #[arg(long = "merge-context")]
    pub merge_context: bool,

    // ── AI Agent Options ────────────────────────────────────────────
    /// Maximum tokens in output (uses heuristic estimation).
    #[arg(long = "token-budget")]
    pub token_budget: Option<u64>,

    /// Enable relevance ranking (default for json format).
    #[arg(long)]
    pub rank: bool,

    /// Disable relevance ranking.
    #[arg(long = "no-rank")]
    pub no_rank: bool,

    /// Collapse near-duplicate results.
    #[arg(long)]
    pub dedup: bool,

    /// Maximum number of results to return.
    #[arg(long = "max-results", default_value = "100")]
    pub max_results: usize,

    // ── Meta ────────────────────────────────────────────────────────
    /// Number of search threads (default: auto-detect).
    #[arg(short = 'j', long = "threads")]
    pub threads: Option<usize>,

    /// Print debug/timing info to stderr.
    #[arg(long)]
    pub debug: bool,
}

/// Parse a filesize string like "1M", "500K", "1024" into bytes.
pub fn parse_filesize(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();
    if let Some(num) = s.strip_suffix('G') {
        num.parse::<u64>().ok().map(|n| n * 1024 * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('M') {
        num.parse::<u64>().ok().map(|n| n * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix('K') {
        num.parse::<u64>().ok().map(|n| n * 1024)
    } else {
        s.parse::<u64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filesize() {
        assert_eq!(parse_filesize("1M"), Some(1024 * 1024));
        assert_eq!(parse_filesize("500K"), Some(500 * 1024));
        assert_eq!(parse_filesize("1024"), Some(1024));
        assert_eq!(parse_filesize("2G"), Some(2 * 1024 * 1024 * 1024));
    }
}
