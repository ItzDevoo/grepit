//! Fixed and adaptive context extraction.

use grepit_ranker::ScoredMatch;
use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for context extraction.
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Lines of context before each match.
    pub before: usize,
    /// Lines of context after each match.
    pub after: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            before: 2,
            after: 2,
        }
    }
}

/// A match with its surrounding context lines.
#[derive(Debug, Clone)]
pub struct ContextualMatch {
    pub scored: ScoredMatch,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// Extract context for a batch of scored matches.
///
/// Reads each unique file once and extracts context for all matches in that file.
/// This is much more efficient than reading each file per-match.
pub fn extract_context(matches: Vec<ScoredMatch>, config: &ContextConfig) -> Vec<ContextualMatch> {
    if config.before == 0 && config.after == 0 {
        // No context requested — fast path
        return matches
            .into_iter()
            .map(|scored| ContextualMatch {
                scored,
                context_before: Vec::new(),
                context_after: Vec::new(),
            })
            .collect();
    }

    // Group matches by file path for efficient reading
    let mut by_file: HashMap<PathBuf, Vec<(usize, ScoredMatch)>> = HashMap::new();
    for (idx, m) in matches.into_iter().enumerate() {
        by_file
            .entry(m.raw.path.clone())
            .or_default()
            .push((idx, m));
    }

    let mut results: Vec<(usize, ContextualMatch)> = Vec::new();

    for (path, file_matches) in by_file {
        // Read the file once
        let lines = match std::fs::read_to_string(&path) {
            Ok(content) => content.lines().map(|l| l.to_string()).collect::<Vec<_>>(),
            Err(_) => {
                // If we can't read the file, return matches without context
                for (idx, scored) in file_matches {
                    results.push((
                        idx,
                        ContextualMatch {
                            scored,
                            context_before: Vec::new(),
                            context_after: Vec::new(),
                        },
                    ));
                }
                continue;
            }
        };

        for (idx, scored) in file_matches {
            let line_idx = (scored.raw.line_number as usize).saturating_sub(1);

            let start = line_idx.saturating_sub(config.before);
            let end = (line_idx + config.after + 1).min(lines.len());

            let context_before = if start < line_idx {
                lines[start..line_idx].to_vec()
            } else {
                Vec::new()
            };

            let context_after = if line_idx + 1 < end {
                lines[line_idx + 1..end].to_vec()
            } else {
                Vec::new()
            };

            results.push((
                idx,
                ContextualMatch {
                    scored,
                    context_before,
                    context_after,
                },
            ));
        }
    }

    // Restore original order
    results.sort_by_key(|(idx, _)| *idx);
    results.into_iter().map(|(_, m)| m).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use grepit_ranker::SignalSet;
    use grepit_searcher::RawMatch;
    use std::path::PathBuf;

    #[test]
    fn test_no_context_fast_path() {
        let config = ContextConfig {
            before: 0,
            after: 0,
        };
        let matches = vec![ScoredMatch {
            raw: RawMatch {
                path: PathBuf::from("nonexistent.txt"),
                line_number: 1,
                column: 1,
                line_content: "test".to_string(),
                match_text: "test".to_string(),
                file_line_count: 1,
            },
            score: 0.5,
            signals: SignalSet::default(),
        }];

        let result = extract_context(matches, &config);
        assert_eq!(result.len(), 1);
        assert!(result[0].context_before.is_empty());
        assert!(result[0].context_after.is_empty());
    }
}
