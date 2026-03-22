//! Individual scoring signals that contribute to match relevance.

use std::path::Path;
use grepit_searcher::RawMatch;

/// The set of scoring signals computed for a match.
#[derive(Debug, Clone, Default)]
pub struct SignalSet {
    /// 1.0 if match is at word boundaries on both sides.
    pub word_boundary: f32,
    /// Score based on path quality (penalizes test/vendor dirs).
    pub path_relevance: f32,
    /// Score based on file type (source > config > docs).
    pub file_type: f32,
    /// Score based on line position (top of file = slight boost).
    pub line_position: f32,
    /// Whether the matched line looks like a definition.
    pub is_definition: f32,
}

impl SignalSet {
    /// Compute all signals for a given match.
    pub fn compute(raw: &RawMatch) -> Self {
        Self {
            word_boundary: compute_word_boundary(raw),
            path_relevance: compute_path_relevance(&raw.path),
            file_type: compute_file_type_score(&raw.path),
            line_position: compute_line_position(raw),
            is_definition: compute_definition_signal(raw),
        }
    }

    /// Combine signals into a final score using weighted sum.
    pub fn score(&self) -> f32 {
        const W_WORD: f32 = 0.15;
        const W_PATH: f32 = 0.25;
        const W_TYPE: f32 = 0.10;
        const W_LINE: f32 = 0.05;
        const W_DEF: f32 = 0.45;

        let raw = W_WORD * self.word_boundary
            + W_PATH * self.path_relevance
            + W_TYPE * self.file_type
            + W_LINE * self.line_position
            + W_DEF * self.is_definition;

        raw.clamp(0.0, 1.0)
    }
}

/// Check if the match text is bounded by word boundaries.
fn compute_word_boundary(raw: &RawMatch) -> f32 {
    let line = &raw.line_content;
    let col = (raw.column - 1) as usize;
    let match_end = col + raw.match_text.len();

    let left_boundary = col == 0
        || line.as_bytes().get(col - 1).is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
    let right_boundary = match_end >= line.len()
        || line
            .as_bytes()
            .get(match_end)
            .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');

    if left_boundary && right_boundary {
        1.0
    } else {
        0.3
    }
}

/// Score path relevance — penalize test/vendor/generated paths.
fn compute_path_relevance(path: &Path) -> f32 {
    let path_str = path.to_str().unwrap_or("").to_lowercase();

    // Heavy penalties
    if path_str.contains("node_modules")
        || path_str.contains("vendor")
        || path_str.contains("__pycache__")
    {
        return 0.0;
    }

    // Moderate penalties
    let mut score: f32 = 0.7;

    if path_str.contains("test") || path_str.contains("spec") || path_str.contains("mock") {
        score -= 0.3;
    }
    if path_str.contains("example") || path_str.contains("sample") {
        score -= 0.2;
    }
    if path_str.contains("generated") || path_str.contains("auto_generated") {
        score -= 0.4;
    }

    // Boosts
    if path_str.contains("src/") || path_str.contains("lib/") || path_str.contains("core/") {
        score += 0.3;
    }

    score.clamp(0.0, 1.0)
}

/// Score based on whether the file is source code, config, docs, etc.
fn compute_file_type_score(path: &Path) -> f32 {
    let ft = grepit_searcher::classify_file_type(path);
    if ft.is_source() {
        1.0
    } else {
        match ft {
            grepit_searcher::FileType::Json
            | grepit_searcher::FileType::Yaml
            | grepit_searcher::FileType::Toml => 0.5,
            grepit_searcher::FileType::Markdown => 0.3,
            _ => 0.4,
        }
    }
}

/// Slight boost for matches near the top of the file (imports, declarations).
fn compute_line_position(raw: &RawMatch) -> f32 {
    if raw.file_line_count == 0 {
        return 0.5;
    }
    let relative_pos = raw.line_number as f32 / raw.file_line_count as f32;
    // Top 20% of file gets a boost
    if relative_pos <= 0.2 {
        1.0
    } else if relative_pos <= 0.5 {
        0.7
    } else {
        0.4
    }
}

/// Heuristic check if a line looks like a definition (without tree-sitter).
fn compute_definition_signal(raw: &RawMatch) -> f32 {
    let line = raw.line_content.trim();

    // Common definition patterns across languages
    let def_patterns = [
        // Rust
        "fn ", "pub fn ", "struct ", "pub struct ", "enum ", "pub enum ",
        "trait ", "pub trait ", "impl ", "type ", "pub type ", "const ",
        "pub const ", "static ", "pub static ", "mod ", "pub mod ",
        // Python
        "def ", "class ", "async def ",
        // JavaScript/TypeScript
        "function ", "const ", "let ", "var ", "export ", "interface ",
        "export default ", "export function ", "export class ",
        "export const ", "export interface ",
        // Go
        "func ", "type ",
        // Java/C++
        "public class ", "private class ", "protected class ",
        "public static ", "public interface ",
        // Ruby
        "module ",
    ];

    for pat in &def_patterns {
        if line.starts_with(pat) {
            return 1.0;
        }
    }

    // Partial indicators
    if line.contains("= function") || line.contains("=> {") || line.contains("=> (") {
        return 0.7;
    }

    0.2
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_path_relevance_src() {
        let score = compute_path_relevance(&PathBuf::from("src/main.rs"));
        assert!(score > 0.8);
    }

    #[test]
    fn test_path_relevance_test() {
        let score = compute_path_relevance(&PathBuf::from("tests/test_main.rs"));
        assert!(score < 0.6);
    }

    #[test]
    fn test_definition_signal() {
        let raw = RawMatch {
            path: PathBuf::from("src/lib.rs"),
            line_number: 1,
            column: 1,
            line_content: "pub fn process_data(input: &str) -> Result<()> {".to_string(),
            match_text: "process_data".to_string(),
            file_line_count: 100,
        };
        let signal = compute_definition_signal(&raw);
        assert_eq!(signal, 1.0);
    }

    #[test]
    fn test_non_definition_signal() {
        let raw = RawMatch {
            path: PathBuf::from("src/lib.rs"),
            line_number: 50,
            column: 10,
            line_content: "    process_data(input)?;".to_string(),
            match_text: "process_data".to_string(),
            file_line_count: 100,
        };
        let signal = compute_definition_signal(&raw);
        assert!(signal < 0.5);
    }
}
