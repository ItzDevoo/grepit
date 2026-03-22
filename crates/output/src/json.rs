//! JSON output formatter — the primary output format for AI agents.

use crate::OutputConfig;
use grep4ai_context::ContextualMatch;
use grep4ai_tokens::BudgetEnforcer;
use serde::Serialize;
use std::collections::HashMap;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain: Option<Vec<String>>,
}

/// Context lines surrounding a match.
#[derive(Debug, Serialize)]
pub struct ContextBlock {
    pub before: Vec<String>,
    #[serde(rename = "match_line")]
    pub matched_line: String,
    pub after: Vec<String>,
}

/// A file with its match count, for the top_files stat.
#[derive(Debug, Serialize)]
pub struct TopFile {
    pub path: String,
    pub match_count: u64,
}

/// Statistics about the search.
#[derive(Debug, Serialize)]
pub struct SearchStats {
    pub search_succeeded: bool,
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
    /// Number of high-relevance results (score > 0.7) that were skipped
    /// during greedy packing due to token budget constraints.
    #[serde(skip_serializing_if = "is_zero")]
    pub skipped_high_relevance_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_files: Option<Vec<TopFile>>,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

/// Normalize path separators to forward slashes on all platforms.
fn normalize_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Convert contextual matches to JSON search results.
fn to_search_results(matches: &[ContextualMatch], explain: bool) -> Vec<SearchResult> {
    matches
        .iter()
        .map(|m| {
            let file_type = grep4ai_searcher::classify_file_type(&m.scored.raw.path);
            SearchResult {
                path: normalize_path(&m.scored.raw.path),
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
                explain: if explain {
                    Some(m.scored.signals.explain())
                } else {
                    None
                },
            }
        })
        .collect()
}

/// Compute top files by match count from the full match set.
fn compute_top_files(matches: &[ContextualMatch], limit: usize) -> Vec<TopFile> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    for m in matches {
        let path = normalize_path(&m.scored.raw.path);
        *counts.entry(path).or_insert(0) += 1;
    }

    let mut files: Vec<TopFile> = counts
        .into_iter()
        .map(|(path, match_count)| TopFile { path, match_count })
        .collect();

    files.sort_by(|a, b| b.match_count.cmp(&a.match_count));
    files.truncate(limit);
    files
}

/// Write JSON output, with greedy token budget packing.
///
/// Instead of truncating at the first result that doesn't fit,
/// greedy packing tries ALL remaining results to maximize information
/// density within the budget. A dense file with 50 short matches
/// shouldn't evict a critical definition just because it came later.
pub fn write_json<W: Write>(
    writer: &mut W,
    matches: Vec<ContextualMatch>,
    files_searched: u64,
    files_skipped: u64,
    total_matches: u64,
    duration_ms: u64,
    config: &OutputConfig,
) -> anyhow::Result<()> {
    // Compute top files before any filtering
    let top_files = if config.show_stats {
        Some(compute_top_files(&matches, 10))
    } else {
        None
    };

    let mut results = to_search_results(&matches, config.explain);
    let mut truncated = false;
    let mut tokens_used: Option<u64> = None;
    let mut skipped_high_relevance_count: u64 = 0;

    // Apply token budget with greedy packing
    if let Some(budget) = config.token_budget {
        let mut enforcer = BudgetEnforcer::new(budget);
        let mut kept = Vec::new();

        // Greedy packing: try every result, skip ones that don't fit,
        // keep going to find smaller results that do fit
        for result in results {
            let serialized = serde_json::to_string(&result)?;
            if enforcer.try_add(&serialized) {
                kept.push(result);
            } else {
                truncated = true;
                // Track when important results are dropped
                if result.score > 0.7 {
                    skipped_high_relevance_count += 1;
                }
                // Don't break — keep trying remaining results (greedy packing)
            }
        }

        tokens_used = Some(enforcer.tokens_used());
        results = kept;
    }

    let response = SearchResponse {
        stats: if config.show_stats {
            Some(SearchStats {
                search_succeeded: true,
                total_matches,
                results_returned: results.len(),
                files_searched,
                files_skipped,
                duration_ms,
                tokens_used,
                token_budget: config.token_budget,
                truncated,
                skipped_high_relevance_count,
                top_files,
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

#[cfg(test)]
mod tests {
    use super::*;
    use grep4ai_context::ContextualMatch;
    use grep4ai_ranker::{ScoredMatch, SignalSet};
    use grep4ai_searcher::RawMatch;
    use std::path::PathBuf;

    fn make_contextual_match(path: &str, line: &str, score: f32) -> ContextualMatch {
        ContextualMatch {
            scored: ScoredMatch {
                raw: RawMatch {
                    path: PathBuf::from(path),
                    line_number: 1,
                    column: 1,
                    line_content: line.to_string(),
                    match_text: "test".to_string(),
                    file_line_count: 10,
                },
                score,
                signals: SignalSet::default(),
            },
            context_before: vec![],
            context_after: vec![],
        }
    }

    #[test]
    fn test_greedy_packing_vs_truncation() {
        // Create matches with varying sizes and scores.
        // The key insight: greedy packing skips big results that don't fit
        // and keeps trying smaller ones, unlike simple truncation which stops.
        let big_line = "x".repeat(300);
        let matches = vec![
            make_contextual_match("a.rs", "short line a", 0.9), // small, fits
            make_contextual_match("b.rs", &big_line, 0.8),      // BIG, won't fit
            make_contextual_match("c.rs", "short line c", 0.7), // small, fits after skip
            make_contextual_match("d.rs", "short line d", 0.6), // small, fits after skip
        ];

        // Budget big enough for ~3 small results but not the big one
        let config = OutputConfig {
            token_budget: Some(300),
            explain: false,
            ..Default::default()
        };

        let mut buf = Vec::new();
        write_json(&mut buf, matches, 10, 0, 4, 100, &config).unwrap();
        let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();

        let results = output["results"].as_array().unwrap();

        // Greedy packing should include a.rs, skip b.rs (too big), then include c.rs and d.rs
        // Simple truncation would include a.rs, fail on b.rs, and stop.
        assert!(
            results.len() >= 2,
            "Greedy packing should include at least 2 results (skipping big one), got {}",
            results.len()
        );

        // Verify the total relevance score is higher than just the first result
        let total_score: f64 = results.iter().map(|r| r["score"].as_f64().unwrap()).sum();
        assert!(
            total_score > 0.9,
            "Total score should exceed first result's score"
        );
    }

    #[test]
    fn test_path_separators_normalized() {
        let match_with_backslash =
            make_contextual_match("src\\auth\\login.rs", "pub fn authenticate() {", 0.9);

        let config = OutputConfig {
            show_stats: false,
            ..Default::default()
        };

        let mut buf = Vec::new();
        write_json(&mut buf, vec![match_with_backslash], 1, 0, 1, 10, &config).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Should not contain backslashes in paths
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let path = parsed["results"][0]["path"].as_str().unwrap();
        assert!(
            !path.contains('\\'),
            "Path should use forward slashes, got: {path}"
        );
        assert!(
            path.contains('/'),
            "Path should contain forward slashes: {path}"
        );
    }

    #[test]
    fn test_search_succeeded_in_stats() {
        let config = OutputConfig::default();
        let mut buf = Vec::new();
        write_json(&mut buf, vec![], 5, 0, 0, 10, &config).unwrap();
        let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();

        assert_eq!(output["stats"]["search_succeeded"], true);
    }

    #[test]
    fn test_token_budget_tolerance() {
        // Generate many code-like results
        let matches: Vec<ContextualMatch> = (0..50)
            .map(|i| {
                make_contextual_match(
                    &format!("src/mod{i}.rs"),
                    &format!("pub fn handler_{i}(req: Request) -> Response {{"),
                    0.5,
                )
            })
            .collect();

        let budget = 1000u64;
        let config = OutputConfig {
            token_budget: Some(budget),
            ..Default::default()
        };

        let mut buf = Vec::new();
        write_json(&mut buf, matches, 50, 0, 50, 100, &config).unwrap();
        let output: serde_json::Value = serde_json::from_slice(&buf).unwrap();

        let tokens_used = output["stats"]["tokens_used"].as_u64().unwrap();
        let max_allowed = (budget as f64 * 1.15) as u64;
        assert!(
            tokens_used <= max_allowed,
            "Token budget {budget} should produce at most {max_allowed} tokens, got {tokens_used}"
        );
    }
}
