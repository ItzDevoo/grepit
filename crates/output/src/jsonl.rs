//! JSON Lines (streaming) output formatter.
//!
//! Emits one JSON object per line — ideal for streaming to agents
//! that process results incrementally.

use std::io::Write;

use grepit_context::ContextualMatch;
use grepit_tokens::BudgetEnforcer;
use crate::OutputConfig;
use crate::json::{SearchResult, ContextBlock};

/// Write JSONL output (one JSON object per line).
pub fn write_jsonl<W: Write>(
    writer: &mut W,
    matches: Vec<ContextualMatch>,
    config: &OutputConfig,
) -> anyhow::Result<()> {
    let mut enforcer = config
        .token_budget
        .map(BudgetEnforcer::new);

    for m in &matches {
        let file_type = grepit_searcher::classify_file_type(&m.scored.raw.path);
        let result = SearchResult {
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
        };

        let line = serde_json::to_string(&result)?;

        if let Some(ref mut enf) = enforcer {
            if !enf.try_add(&line) {
                break;
            }
        }

        writeln!(writer, "{line}")?;
    }

    Ok(())
}
