//! Content-type-aware token estimator.
//!
//! Estimates tokens using whitespace-separated word counts with a multiplier
//! based on content type (code vs prose). Code has shorter tokens on average
//! (~2.5 chars/token due to identifiers, brackets, operators) while prose
//! has longer tokens (~4.5 chars/token).

/// Estimate the token count for a string using content-type-aware heuristics.
///
/// 1. Count whitespace-separated words
/// 2. Detect whether content is code or prose
/// 3. Apply a chars-per-token multiplier based on content type
pub fn estimate_tokens(text: &str) -> u64 {
    if text.is_empty() {
        return 0;
    }

    let chars = text.len() as f64;
    let chars_per_token = if is_code(text) { 2.5 } else { 4.5 };

    (chars / chars_per_token).ceil() as u64
}

/// Detect whether text looks like code based on character density of
/// brackets, semicolons, and operators.
fn is_code(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let total = text.len() as f64;
    let code_chars = text
        .bytes()
        .filter(|&b| {
            matches!(
                b,
                b'{' | b'}'
                    | b'('
                    | b')'
                    | b'['
                    | b']'
                    | b';'
                    | b'='
                    | b'<'
                    | b'>'
                    | b'|'
                    | b'&'
                    | b'!'
                    | b'+'
                    | b'-'
                    | b'*'
                    | b'/'
                    | b':'
                    | b','
                    | b'.'
                    | b'_'
            )
        })
        .count() as f64;

    // If more than 8% of characters are code-like punctuation, it's code
    code_chars / total > 0.08
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        // "Hello, world!" is 13 chars, prose → 13/4.5 ≈ 3 tokens
        let estimate = estimate_tokens("Hello, world!");
        assert!(estimate >= 2 && estimate <= 6);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_code_detection() {
        assert!(is_code("fn main() { let x = 42; }"));
        assert!(is_code("if (x > 0) { return x + 1; }"));
        assert!(!is_code(
            "This is a normal English sentence about programming"
        ));
        assert!(!is_code("The quick brown fox jumps over the lazy dog"));
    }

    #[test]
    fn test_code_gets_more_tokens() {
        let code = "fn process_data(input: &str) -> Result<Vec<u8>> { Ok(vec![]) }";
        let prose = "This function processes data from a string input and returns bytes";

        let code_tokens = estimate_tokens(code);
        let prose_tokens = estimate_tokens(prose);

        // Code should produce more tokens per character
        let code_ratio = code.len() as f64 / code_tokens as f64;
        let prose_ratio = prose.len() as f64 / prose_tokens as f64;

        assert!(
            code_ratio < prose_ratio,
            "code ratio ({code_ratio}) should be less than prose ratio ({prose_ratio})"
        );
    }

    #[test]
    fn test_budget_1000_tokens_tolerance() {
        // Generate a mix of code content that would be ~1000 tokens
        // With code estimator at 2.5 chars/token, 2500 chars should be ~1000 tokens
        let code_line = "fn check(x: &str) -> bool { x.len() > 0 }\n";
        let mut text = String::new();
        // Build up to just under 1000 tokens worth of content
        while estimate_tokens(&text) < 1000 {
            text.push_str(code_line);
        }

        let estimated = estimate_tokens(&text);
        // The estimated count should be reasonably close to actual length / 2.5
        // We verify the estimate is within bounds
        assert!(
            estimated <= 1150,
            "budget of 1000 tokens should never produce more than 1150 tokens, got {estimated}"
        );
    }
}
