//! Output formatters for grepit.
//!
//! Provides JSON (default), JSONL (streaming), compact, and human-readable
//! output formats. JSON is the primary format, optimized for AI agent consumption.

mod compact;
mod human;
mod json;
mod jsonl;

pub use json::{ContextBlock, SearchResponse, SearchResult, SearchStats};

use grepit_context::ContextualMatch;

use std::io::Write;

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    JsonLines,
    Compact,
    Human,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "jsonl" | "jsonlines" => Ok(Self::JsonLines),
            "compact" => Ok(Self::Compact),
            "human" => Ok(Self::Human),
            _ => Err(format!(
                "Unknown output format: '{s}'. Expected: json, jsonl, compact, human"
            )),
        }
    }
}

/// Configuration for the output formatter.
pub struct OutputConfig {
    pub format: OutputFormat,
    pub pretty: bool,
    pub show_stats: bool,
    pub token_budget: Option<u64>,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::Json,
            pretty: false,
            show_stats: true,
            token_budget: None,
        }
    }
}

/// Format and write search results to the given writer.
pub fn write_output<W: Write>(
    writer: &mut W,
    matches: Vec<ContextualMatch>,
    files_searched: u64,
    files_skipped: u64,
    total_matches: u64,
    duration_ms: u64,
    config: &OutputConfig,
) -> anyhow::Result<()> {
    match config.format {
        OutputFormat::Json => json::write_json(
            writer,
            matches,
            files_searched,
            files_skipped,
            total_matches,
            duration_ms,
            config,
        ),
        OutputFormat::JsonLines => jsonl::write_jsonl(writer, matches, config),
        OutputFormat::Compact => compact::write_compact(writer, matches, config),
        OutputFormat::Human => human::write_human(
            writer,
            matches,
            files_searched,
            total_matches,
            duration_ms,
            config,
        ),
    }
}
