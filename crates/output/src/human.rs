//! Human-readable colored output.
//!
//! Looks similar to ripgrep's default output for debugging/manual use.

use crate::OutputConfig;
use grepit_context::ContextualMatch;
use std::io::Write;

/// Write human-readable output with file headers and line numbers.
pub fn write_human<W: Write>(
    writer: &mut W,
    matches: Vec<ContextualMatch>,
    files_searched: u64,
    total_matches: u64,
    duration_ms: u64,
    config: &OutputConfig,
) -> anyhow::Result<()> {
    let mut current_file: Option<String> = None;

    for m in &matches {
        let path = m.scored.raw.path.to_string_lossy().to_string();

        // Print file header when we switch files
        if current_file.as_ref() != Some(&path) {
            if current_file.is_some() {
                writeln!(writer)?;
            }
            writeln!(writer, "\x1b[35m{path}\x1b[0m")?;
            current_file = Some(path);
        }

        // Print context before
        for (i, line) in m.context_before.iter().enumerate() {
            let line_num =
                m.scored.raw.line_number as i64 - m.context_before.len() as i64 + i as i64;
            writeln!(writer, "\x1b[32m{line_num}\x1b[0m-{line}")?;
        }

        // Print the matched line
        let line_number = m.scored.raw.line_number;
        let line_content = &m.scored.raw.line_content;
        writeln!(
            writer,
            "\x1b[32m{line_number}\x1b[0m:\x1b[1m\x1b[31m{line_content}\x1b[0m",
        )?;

        // Print context after
        for (i, line) in m.context_after.iter().enumerate() {
            let line_num = m.scored.raw.line_number + 1 + i as u64;
            writeln!(writer, "\x1b[32m{line_num}\x1b[0m-{line}")?;
        }
    }

    if config.show_stats {
        writeln!(writer)?;
        writeln!(
            writer,
            "\x1b[36m{total_matches} matches across {files_searched} files in {duration_ms}ms\x1b[0m",
        )?;
    }

    Ok(())
}
