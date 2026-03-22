//! Relevance scoring algorithm.

use crate::signals::SignalSet;
use grepit_searcher::RawMatch;

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
}

impl Default for RankConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: Some(100),
        }
    }
}

/// Score and rank a set of raw matches.
/// Returns matches sorted by relevance (highest score first).
pub fn rank_matches(matches: Vec<RawMatch>, config: &RankConfig) -> Vec<ScoredMatch> {
    let mut scored: Vec<ScoredMatch> = matches
        .into_iter()
        .map(|raw| {
            let signals = SignalSet::compute(&raw);
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

        let config = RankConfig::default();
        let ranked = rank_matches(matches, &config);

        // The struct definition should rank higher than the usage
        assert_eq!(ranked[0].raw.path.to_str().unwrap(), "src/config.rs");
    }

    #[test]
    fn test_max_results() {
        let matches: Vec<RawMatch> = (0..200)
            .map(|i| make_match("src/lib.rs", &format!("line {}", i), i + 1))
            .collect();

        let config = RankConfig {
            enabled: false,
            max_results: Some(50),
        };
        let ranked = rank_matches(matches, &config);
        assert_eq!(ranked.len(), 50);
    }
}
