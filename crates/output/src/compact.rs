//! Compact output — minimal token usage for context-constrained agents.
//!
//! Format: `path:line:match_text`
//! One match per line, no context, no formatting overhead.

use crate::OutputConfig;
use grepit_context::ContextualMatch;
use grepit_tokens::BudgetEnforcer;
use std::io::Write;

/// Write compact output (path:line:match_text).
pub fn write_compact<W: Write>(
    writer: &mut W,
    matches: Vec<ContextualMatch>,
    config: &OutputConfig,
) -> anyhow::Result<()> {
    let mut enforcer = config.token_budget.map(BudgetEnforcer::new);

    for m in &matches {
        let line = format!(
            "{}:{}:{}",
            m.scored.raw.path.display(),
            m.scored.raw.line_number,
            m.scored.raw.line_content.trim(),
        );

        if let Some(ref mut enf) = enforcer {
            if !enf.try_add(&line) {
                break;
            }
        }

        writeln!(writer, "{line}")?;
    }

    Ok(())
}
