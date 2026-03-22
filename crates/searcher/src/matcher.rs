//! Regex matching with position tracking.

use std::path::{Path, PathBuf};

/// A single match found in a file.
#[derive(Debug, Clone)]
pub struct RawMatch {
    /// Path to the file containing the match.
    pub path: PathBuf,
    /// 1-based line number.
    pub line_number: u64,
    /// 1-based column number (byte offset within the line).
    pub column: u64,
    /// The full content of the matched line (trimmed of trailing newline).
    pub line_content: String,
    /// The actual matched text (the substring that matched the pattern).
    pub match_text: String,
    /// All lines from the file (for context extraction later).
    /// This is stored as an index into a shared line cache, not the lines themselves.
    pub file_line_count: u64,
}

/// Search a file's content (as bytes) for matches against a compiled regex.
/// Returns all matches found.
pub fn find_matches(
    path: &Path,
    content: &[u8],
    regex: &regex::Regex,
    max_count: Option<usize>,
) -> Vec<RawMatch> {
    let mut matches = Vec::new();

    // Convert to string, skipping files that aren't valid UTF-8
    let text = match std::str::from_utf8(content) {
        Ok(t) => t,
        Err(_) => return matches,
    };

    let lines: Vec<&str> = text.lines().collect();
    let file_line_count = lines.len() as u64;

    for (line_idx, line) in lines.iter().enumerate() {
        if let Some(max) = max_count {
            if matches.len() >= max {
                break;
            }
        }

        if let Some(m) = regex.find(line) {
            matches.push(RawMatch {
                path: path.to_path_buf(),
                line_number: (line_idx + 1) as u64,
                column: (m.start() + 1) as u64,
                line_content: line.to_string(),
                match_text: m.as_str().to_string(),
                file_line_count,
            });
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matches_basic() {
        let regex = regex::Regex::new("hello").unwrap();
        let content = b"say hello world\ngoodbye\nhello again";
        let path = PathBuf::from("test.txt");
        let matches = find_matches(&path, content, &regex, None);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_number, 1);
        assert_eq!(matches[0].column, 5);
        assert_eq!(matches[0].match_text, "hello");
        assert_eq!(matches[1].line_number, 3);
    }

    #[test]
    fn test_find_matches_max_count() {
        let regex = regex::Regex::new("a").unwrap();
        let content = b"a\na\na\na";
        let path = PathBuf::from("test.txt");
        let matches = find_matches(&path, content, &regex, Some(2));
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_find_matches_no_match() {
        let regex = regex::Regex::new("xyz").unwrap();
        let content = b"hello world";
        let path = PathBuf::from("test.txt");
        let matches = find_matches(&path, content, &regex, None);
        assert!(matches.is_empty());
    }
}
