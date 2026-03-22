//! Merge overlapping context regions within the same file.

use crate::window::ContextualMatch;
use std::collections::HashMap;
use std::path::PathBuf;

/// Merge overlapping context for matches within the same file.
/// When two matches are close together, their contexts would overlap —
/// this function deduplicates the overlapping lines.
pub fn merge_overlapping(matches: Vec<ContextualMatch>) -> Vec<ContextualMatch> {
    if matches.len() <= 1 {
        return matches;
    }

    // Group by file
    let mut by_file: HashMap<PathBuf, Vec<(usize, ContextualMatch)>> = HashMap::new();
    let non_file_results: Vec<(usize, ContextualMatch)> = Vec::new();

    for (idx, m) in matches.into_iter().enumerate() {
        by_file
            .entry(m.scored.raw.path.clone())
            .or_default()
            .push((idx, m));
    }

    let mut all_results: Vec<(usize, ContextualMatch)> = Vec::new();

    for (_path, mut file_matches) in by_file {
        if file_matches.len() <= 1 {
            all_results.extend(file_matches);
            continue;
        }

        // Sort by line number within the file
        file_matches.sort_by_key(|(_, m)| m.scored.raw.line_number);

        // For adjacent matches, trim the after-context of the first
        // and the before-context of the second to avoid overlap
        for i in 0..file_matches.len() - 1 {
            let current_line = file_matches[i].1.scored.raw.line_number;
            let next_line = file_matches[i + 1].1.scored.raw.line_number;
            let gap = (next_line - current_line) as usize;

            let after_len = file_matches[i].1.context_after.len();
            let before_len = file_matches[i + 1].1.context_before.len();

            if gap <= after_len + before_len + 1 {
                // Overlap detected — split the gap between after and before
                let half = gap.saturating_sub(1) / 2;
                file_matches[i].1.context_after.truncate(half);
                let before = &mut file_matches[i + 1].1.context_before;
                let skip = before.len().saturating_sub(gap.saturating_sub(1) - half);
                *before = before.split_off(skip.min(before.len()));
            }
        }

        all_results.extend(file_matches);
    }

    all_results.extend(non_file_results);
    all_results.sort_by_key(|(idx, _)| *idx);
    all_results.into_iter().map(|(_, m)| m).collect()
}
