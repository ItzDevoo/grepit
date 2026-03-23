//! Individual scoring signals that contribute to match relevance.

use grep4ai_searcher::RawMatch;
use std::path::Path;

/// The set of scoring signals computed for a match.
#[derive(Debug, Clone, Default)]
pub struct SignalSet {
    /// 1.0 if match is at word boundaries on both sides.
    pub word_boundary: f32,
    /// Score based on path quality (penalizes test/vendor dirs).
    pub path_relevance: f32,
    /// Boost when the file path contains tokens from the query.
    pub query_path_boost: f32,
    /// Score based on file type (source > config > docs).
    pub file_type: f32,
    /// Score based on line position (top of file = slight boost).
    pub line_position: f32,
    /// Whether the matched line looks like a definition.
    pub is_definition: f32,
}

impl SignalSet {
    /// Compute all signals for a given match.
    pub fn compute(raw: &RawMatch, query: &str) -> Self {
        Self {
            word_boundary: compute_word_boundary(raw),
            path_relevance: compute_path_relevance(&raw.path),
            query_path_boost: compute_query_path_boost(&raw.path, query),
            file_type: compute_file_type_score(&raw.path),
            line_position: compute_line_position(raw),
            is_definition: compute_definition_signal(raw),
        }
    }

    /// Combine signals into a final score using weighted sum.
    pub fn score(&self) -> f32 {
        const W_WORD: f32 = 0.10;
        const W_PATH: f32 = 0.15;
        const W_QPATH: f32 = 0.15;
        const W_TYPE: f32 = 0.10;
        const W_LINE: f32 = 0.05;
        const W_DEF: f32 = 0.45;

        let raw = W_WORD * self.word_boundary
            + W_PATH * self.path_relevance
            + W_QPATH * self.query_path_boost
            + W_TYPE * self.file_type
            + W_LINE * self.line_position
            + W_DEF * self.is_definition;

        raw.clamp(0.0, 1.0)
    }

    /// Human-readable explanation of why this result scored the way it did.
    pub fn explain(&self) -> Vec<String> {
        let mut reasons = Vec::new();

        if self.is_definition >= 0.8 {
            reasons.push("definition (fn/class/struct/type declaration)".to_string());
        } else if self.is_definition >= 0.5 {
            reasons.push("likely definition (arrow fn / assignment)".to_string());
        }

        if self.query_path_boost >= 0.8 {
            reasons.push("file path strongly matches query".to_string());
        } else if self.query_path_boost >= 0.4 {
            reasons.push("file path partially matches query".to_string());
        }

        if self.path_relevance <= 0.2 {
            reasons.push("penalized: vendor/generated path".to_string());
        } else if self.path_relevance <= 0.5 {
            reasons.push("penalized: test/example path".to_string());
        } else if self.path_relevance >= 0.9 {
            reasons.push("core source path (src/lib/core)".to_string());
        }

        if self.word_boundary >= 0.9 {
            reasons.push("exact word match".to_string());
        }

        if self.line_position >= 0.9 {
            reasons.push("near top of file (declaration zone)".to_string());
        }

        if self.file_type >= 0.9 {
            reasons.push("source code file".to_string());
        } else if self.file_type <= 0.4 {
            reasons.push("non-source file (config/docs)".to_string());
        }

        if reasons.is_empty() {
            reasons.push("general match".to_string());
        }

        reasons
    }
}

/// Check if the match text is bounded by word boundaries.
fn compute_word_boundary(raw: &RawMatch) -> f32 {
    let line = &raw.line_content;
    let col = (raw.column - 1) as usize;
    let match_end = col + raw.match_text.len();

    let left_boundary = col == 0
        || line
            .as_bytes()
            .get(col - 1)
            .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
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

/// Boost files whose path contains tokens from the search query.
/// If the query is "authenticate", a file at `src/auth/handler.rs` scores higher.
fn compute_query_path_boost(path: &Path, query: &str) -> f32 {
    let path_str = path.to_str().unwrap_or("").to_lowercase();

    // Extract meaningful tokens from the query (split on non-alphanumeric, filter short ones)
    let query_lower = query.to_lowercase();
    let tokens: Vec<&str> = query_lower
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| t.len() >= 3) // skip short tokens like "fn", "if"
        .collect();

    if tokens.is_empty() {
        return 0.0;
    }

    let mut hits = 0;
    for token in &tokens {
        if path_str.contains(token) {
            hits += 1;
        }
        // Also check if the query token is a substring of a path component
        // e.g., query "authenticate" matches path "auth/"
        if token.len() >= 4 && path_str.contains(&token[..token.len().min(4)]) {
            hits += 1;
        }
    }

    let max_possible = tokens.len() * 2; // full match + prefix match
    let ratio = hits as f32 / max_possible as f32;
    ratio.clamp(0.0, 1.0)
}

/// Score based on whether the file is source code, config, docs, etc.
fn compute_file_type_score(path: &Path) -> f32 {
    let ft = grep4ai_searcher::classify_file_type(path);
    if ft.is_source() {
        1.0
    } else {
        match ft {
            grep4ai_searcher::FileType::Json
            | grep4ai_searcher::FileType::Yaml
            | grep4ai_searcher::FileType::Toml => 0.5,
            grep4ai_searcher::FileType::Markdown => 0.3,
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

/// Check if the match text appears on the left-hand side of an assignment.
/// For `const foo = useState(...)`, "foo" is LHS (definition) and "useState" is RHS (usage).
fn is_match_on_lhs(line: &str, match_text: &str) -> bool {
    if let Some(eq_pos) = line.find('=') {
        // Don't match ==, =>, etc.
        let after_eq = line.as_bytes().get(eq_pos + 1);
        let is_assignment = after_eq.is_none_or(|&b| b != b'=' && b != b'>');
        if is_assignment {
            let lhs = &line[..eq_pos];
            return lhs.contains(match_text);
        }
    }
    false
}

/// Heuristic check if a line looks like a definition.
///
/// Improved: checks that definition keywords appear at the START of the
/// trimmed line (position-aware), not just anywhere. This avoids false
/// positives from comments like `// this function does X` or strings
/// containing definition keywords.
fn compute_definition_signal(raw: &RawMatch) -> f32 {
    let line = raw.line_content.trim();

    // Skip lines that look like comments (but not Python decorators)
    if line.starts_with("//")
        || (line.starts_with('#') && !line.starts_with("#["))
        || line.starts_with("/*")
        || line.starts_with('*')
        || line.starts_with("<!--")
    {
        return 0.1;
    }

    // Python decorators that signal a definition follows
    if line.starts_with("@dataclass")
        || line.starts_with("@staticmethod")
        || line.starts_with("@classmethod")
        || line.starts_with("@property")
        || line.starts_with("@abstractmethod")
    {
        return 0.9;
    }

    // Common definition patterns — must appear at the START of the trimmed line
    let def_patterns = [
        // Rust
        "fn ",
        "pub fn ",
        "pub(crate) fn ",
        "async fn ",
        "pub async fn ",
        "struct ",
        "pub struct ",
        "enum ",
        "pub enum ",
        "trait ",
        "pub trait ",
        "impl ",
        "impl<",
        "type ",
        "pub type ",
        "const ",
        "pub const ",
        "static ",
        "pub static ",
        "mod ",
        "pub mod ",
        // Python
        "def ",
        "class ",
        "async def ",
        // JavaScript/TypeScript
        "function ",
        "async function ",
        "export function ",
        "export async function ",
        "export default function ",
        "export class ",
        "export const ",
        "export interface ",
        "export type ",
        "export enum ",
        "export let ",
        "export var ",
        "export default class ",
        "interface ",
        // Go
        "func ",
        "func (",
        // Java/C#/C++
        "public class ",
        "private class ",
        "protected class ",
        "public static ",
        "public interface ",
        "public enum ",
        "abstract class ",
        // Ruby
        "module ",
    ];

    for pat in &def_patterns {
        if line.starts_with(pat) {
            return 1.0;
        }
    }

    // JS/TS variable declarations that are definitions (const/let/var at start of line)
    // Only count as definition if the match_text is the name being defined,
    // not something on the RHS of the assignment (e.g. `const x = useState()` —
    // `useState` is a usage, `x` is the definition).
    if (line.starts_with("const ") || line.starts_with("let ") || line.starts_with("var "))
        && line.contains('=')
    {
        if is_match_on_lhs(line, &raw.match_text) {
            return 0.8;
        }
        // match_text is on the RHS — this is a usage, not a definition
        return 0.15;
    }

    // Export default with assignment
    if line.starts_with("export default ") {
        return 0.8;
    }

    // Arrow functions / function expressions assigned to variables
    if line.contains("= function")
        || line.contains("=> {")
        || line.contains("=> (")
        || line.contains("= () =>")
        || line.contains("= async (")
    {
        if is_match_on_lhs(line, &raw.match_text) {
            return 0.6;
        }
        return 0.15;
    }

    // Rust attribute macros that define items (#[derive], #[test], etc.)
    if line.starts_with("#[") {
        return 0.5;
    }

    0.15
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

    #[test]
    fn test_comment_not_definition() {
        let raw = RawMatch {
            path: PathBuf::from("src/lib.rs"),
            line_number: 1,
            column: 1,
            line_content: "// fn this_is_a_comment".to_string(),
            match_text: "fn".to_string(),
            file_line_count: 100,
        };
        let signal = compute_definition_signal(&raw);
        assert!(signal < 0.2);
    }

    #[test]
    fn test_query_path_boost() {
        let score = compute_query_path_boost(&PathBuf::from("src/auth/handler.rs"), "authenticate");
        assert!(score > 0.3);
    }

    #[test]
    fn test_query_path_no_match() {
        let score =
            compute_query_path_boost(&PathBuf::from("src/database/pool.rs"), "authenticate");
        assert!(score < 0.1);
    }

    fn make_raw(line: &str) -> RawMatch {
        RawMatch {
            path: PathBuf::from("src/lib.rs"),
            line_number: 1,
            column: 1,
            line_content: line.to_string(),
            match_text: "test".to_string(),
            file_line_count: 100,
        }
    }

    #[test]
    fn test_ts_arrow_export() {
        let raw = make_raw("export const handler = () => {");
        assert!(compute_definition_signal(&raw) >= 0.6);
    }

    #[test]
    fn test_ts_interface() {
        let raw = make_raw("interface UserConfig {");
        assert_eq!(compute_definition_signal(&raw), 1.0);
    }

    #[test]
    fn test_ts_type_alias() {
        let raw = make_raw("type Result<T> = { ok: T } | { err: Error }");
        assert_eq!(compute_definition_signal(&raw), 1.0);
    }

    #[test]
    fn test_python_dataclass() {
        let raw = make_raw("@dataclass");
        assert!(compute_definition_signal(&raw) >= 0.9);
    }

    #[test]
    fn test_python_async_def() {
        let raw = make_raw("async def fetch_data(url: str) -> dict:");
        assert_eq!(compute_definition_signal(&raw), 1.0);
    }

    #[test]
    fn test_rust_impl_block() {
        let raw = make_raw("impl Display for Config {");
        assert_eq!(compute_definition_signal(&raw), 1.0);
    }

    #[test]
    fn test_rust_impl_generic() {
        let raw = make_raw("impl<T: Clone> Widget<T> {");
        assert_eq!(compute_definition_signal(&raw), 1.0);
    }

    #[test]
    fn test_go_method() {
        let raw = make_raw("func (s *Server) HandleRequest(w http.ResponseWriter) {");
        assert_eq!(compute_definition_signal(&raw), 1.0);
    }

    #[test]
    fn test_export_default_function() {
        let raw = make_raw("export default function App() {");
        assert_eq!(compute_definition_signal(&raw), 1.0);
    }

    #[test]
    fn test_const_arrow_fn() {
        let raw = make_raw("const processData = () => {");
        assert!(compute_definition_signal(&raw) >= 0.6);
    }
}
