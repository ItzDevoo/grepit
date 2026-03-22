//! JSON output formatter — the primary output format for AI agents.

use crate::OutputConfig;
use grepit_context::ContextualMatch;
use grepit_tokens::BudgetEnforcer;
use serde::Serialize;
use std::io::Write;

/// The top-level JSON response.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<SearchStats>,
}

/// A single search result in the JSON output.
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub line: u64,
    pub column: u64,
    pub match_text: String,
    pub context: ContextBlock,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
}

/// Context lines surrounding a match.
#[derive(Debug, Serialize)]
pub struct ContextBlock {
    pub before: Vec<String>,
    #[serde(rename = "match_line")]
    pub matched_line: String,
    pub after: Vec<String>,
}

/// Statistics about the search.
#[derive(Debug, Serialize)]
pub struct SearchStats {
    pub total_matches: u64,
    pub results_returned: usize,
    pub files_searched: u64,
    pub files_skipped: u64,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u64>,
    pub truncated: bool,
}

/// Convert contextual matches to JSON search results.
fn to_search_results(matches: &[ContextualMatch]) -> Vec<SearchResult> {
    matches
        .iter()
        .map(|m| {
            let file_type = grepit_searcher::classify_file_type(&m.scored.raw.path);
            SearchResult {
                path: m.scored.raw.path.to_string_lossy().to_string(),
                line: m.scored.raw.line_number,
                column: m.scored.raw.column,
                match_text: m.scored.raw.match_text.clone(),
                context: ContextBlock {
                    before: m.context_before.clone(),
                    matched_line: m.scored.raw.line_content.clone(),
                    after: m.context_after.clone(),
                },
                score: (m.scored.score * 100.0).round() / 100.0,
                file_type: if file_type.name() != "unknown" {
                    Some(file_type.name().to_string())
                } else {
                    None
                },
            }
        })
        .collect()
}

/// Write JSON output, optionally with token budget enforcement.
pub fn write_json<W: Write>(
    writer: &mut W,
    matches: Vec<ContextualMatch>,
    files_searched: u64,
    files_skipped: u64,
    total_matches: u64,
    duration_ms: u64,
    config: &OutputConfig,
) -> anyhow::Result<()> {
    let mut results = to_search_results(&matches);
    let mut truncated = false;
    let mut tokens_used: Option<u64> = None;

    // Apply token budget if configured
    if let Some(budget) = config.token_budget {
        let mut enforcer = BudgetEnforcer::new(budget);
        let mut kept = Vec::new();

        for result in results {
            let serialized = serde_json::to_string(&result)?;
            if enforcer.try_add(&serialized) {
                kept.push(result);
            } else {
                truncated = true;
                break;
            }
        }

        tokens_used = Some(enforcer.tokens_used());
        results = kept;
    }

    let response = SearchResponse {
        stats: if config.show_stats {
            Some(SearchStats {
                total_matches,
                results_returned: results.len(),
                files_searched,
                files_skipped,
                duration_ms,
                tokens_used,
                token_budget: config.token_budget,
                truncated,
            })
        } else {
            None
        },
        results,
    };

    if config.pretty {
        serde_json::to_writer_pretty(&mut *writer, &response)?;
    } else {
        serde_json::to_writer(&mut *writer, &response)?;
    }

    writeln!(writer)?;
    Ok(())
}
