//! Relevance scoring algorithm.

use crate::signals::SignalSet;
use grep4ai_searcher::RawMatch;

/// A match with its computed relevance score.
#[derive(Debug, Clone)]
pub struct ScoredMatch {
    pub raw: RawMatch,
    pub score: f32,
    pub signals: SignalSet,
}

/// Configuration for the ranker.
#[derive(Debug, Clone)]
pub struct RankConfig {
    /// Whether ranking is enabled.
    pub enabled: bool,
    /// Maximum number of results to return.
    pub max_results: Option<usize>,
    /// The search query (used for query-aware path boosting).
    pub query: String,
}

impl Default for RankConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: Some(100),
            query: String::new(),
        }
    }
}

/// Score and rank a set of raw matches.
/// Returns matches sorted by relevance (highest score first).
pub fn rank_matches(matches: Vec<RawMatch>, config: &RankConfig) -> Vec<ScoredMatch> {
    let mut scored: Vec<ScoredMatch> = matches
        .into_iter()
        .map(|raw| {
            let signals = SignalSet::compute(&raw, &config.query);
            let score = signals.score();
            ScoredMatch {
                raw,
                score,
                signals,
            }
        })
        .collect();

    if config.enabled {
        // Sort by score descending (highest relevance first)
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Apply max results limit
    if let Some(max) = config.max_results {
        scored.truncate(max);
    }

    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_match(path: &str, line: &str, line_num: u64) -> RawMatch {
        RawMatch {
            path: PathBuf::from(path),
            line_number: line_num,
            column: 1,
            line_content: line.to_string(),
            match_text: "Config".to_string(),
            file_line_count: 100,
        }
    }

    #[test]
    fn test_definitions_rank_higher() {
        let matches = vec![
            make_match("src/main.rs", "    let config = Config::new();", 50),
            make_match("src/config.rs", "pub struct Config {", 5),
        ];

        let config = RankConfig {
            query: "Config".to_string(),
            ..Default::default()
        };
        let ranked = rank_matches(matches, &config);

        // The struct definition should rank higher than the usage
        assert_eq!(ranked[0].raw.path.to_str().unwrap(), "src/config.rs");
    }

    #[test]
    fn test_query_path_boost() {
        let matches = vec![
            make_match("src/utils.rs", "pub struct Config {", 5),
            make_match("src/config.rs", "pub struct Config {", 5),
        ];

        let config = RankConfig {
            query: "Config".to_string(),
            ..Default::default()
        };
        let ranked = rank_matches(matches, &config);

        // config.rs should rank higher due to query-path boost
        assert_eq!(ranked[0].raw.path.to_str().unwrap(), "src/config.rs");
    }

    #[test]
    fn test_max_results() {
        let matches: Vec<RawMatch> = (0..200)
            .map(|i| make_match("src/lib.rs", &format!("line {i}"), i + 1))
            .collect();

        let config = RankConfig {
            enabled: false,
            max_results: Some(50),
            query: String::new(),
        };
        let ranked = rank_matches(matches, &config);
        assert_eq!(ranked.len(), 50);
    }

    #[test]
    fn test_query_path_boost_authenticate() {
        // A match in auth/login.rs should score higher than utils/helpers.rs
        // when the query is "authenticate"
        let matches = vec![
            RawMatch {
                path: PathBuf::from("src/utils/helpers.rs"),
                line_number: 10,
                column: 1,
                line_content: "pub fn authenticate(user: &str) -> bool {".to_string(),
                match_text: "authenticate".to_string(),
                file_line_count: 100,
            },
            RawMatch {
                path: PathBuf::from("src/auth/login.rs"),
                line_number: 10,
                column: 1,
                line_content: "pub fn authenticate(user: &str) -> bool {".to_string(),
                match_text: "authenticate".to_string(),
                file_line_count: 100,
            },
        ];

        let config = RankConfig {
            query: "authenticate".to_string(),
            ..Default::default()
        };
        let ranked = rank_matches(matches, &config);

        // auth/login.rs should rank higher due to query-path boost
        assert!(
            ranked[0].raw.path.to_str().unwrap().contains("auth"),
            "Expected auth/login.rs to rank first, got: {}",
            ranked[0].raw.path.display()
        );
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn test_explain_output_contains_signal_names() {
        let raw = RawMatch {
            path: PathBuf::from("src/auth/handler.rs"),
            line_number: 5,
            column: 1,
            line_content: "pub fn authenticate(user: &str) -> bool {".to_string(),
            match_text: "authenticate".to_string(),
            file_line_count: 100,
        };

        let signals = SignalSet::compute(&raw, "authenticate");
        let explanation = signals.explain();

        // Should mention it's a definition
        assert!(
            explanation.iter().any(|s| s.contains("definition")),
            "Expected 'definition' in explanation, got: {:?}",
            explanation
        );
        // Should mention path match
        assert!(
            explanation.iter().any(|s| s.contains("path")),
            "Expected 'path' mention in explanation, got: {:?}",
            explanation
        );
    }
}
