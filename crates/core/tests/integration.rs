//! Integration tests for the grep4ai binary.
//!
//! These tests run the actual binary and check exit codes, output format, etc.

use std::process::Command;

fn grep4ai_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_grep4ai"))
}

#[test]
fn test_exit_code_1_no_matches() {
    let output = grep4ai_bin()
        .args(["this_pattern_will_never_match_xyzzy_123", "Cargo.toml"])
        .output()
        .expect("failed to run grep4ai");

    assert_eq!(
        output.status.code(),
        Some(1),
        "Exit code should be 1 when no matches found"
    );
}

#[test]
fn test_exit_code_0_with_matches() {
    let output = grep4ai_bin()
        .args(["grep4ai", "Cargo.toml"])
        .output()
        .expect("failed to run grep4ai");

    assert_eq!(
        output.status.code(),
        Some(0),
        "Exit code should be 0 when matches are found"
    );
}

#[test]
fn test_json_output_has_forward_slashes() {
    let output = grep4ai_bin()
        .args(["-f", "json", "grep4ai", "Cargo.toml"])
        .output()
        .expect("failed to run grep4ai");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse as JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");

    if let Some(results) = parsed["results"].as_array() {
        for result in results {
            let path = result["path"].as_str().unwrap();
            assert!(
                !path.contains('\\'),
                "Path should not contain backslashes: {path}"
            );
        }
    }
}

#[test]
fn test_version_flag() {
    let output = grep4ai_bin()
        .arg("--version")
        .output()
        .expect("failed to run grep4ai");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("grep4ai"),
        "Version output should contain 'grep4ai': {stdout}"
    );
}

#[test]
fn test_find_definitions_regex_escaping() {
    // This test verifies that special regex characters in patterns don't crash grep4ai.
    // Using -F (fixed-string) mode simulates what find_definitions does after escaping.
    let output = grep4ai_bin()
        .args(["-F", "-f", "json", "Array<string>", "Cargo.toml"])
        .output()
        .expect("failed to run grep4ai");

    // Should not crash — exit code 1 (no matches) is fine
    let code = output.status.code().unwrap();
    assert!(
        code == 0 || code == 1,
        "Should exit cleanly (0 or 1), got {code}"
    );
}

#[test]
fn test_search_succeeded_in_output() {
    let output = grep4ai_bin()
        .args(["-f", "json", "grep4ai", "Cargo.toml"])
        .output()
        .expect("failed to run grep4ai");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        parsed["stats"]["search_succeeded"],
        serde_json::Value::Bool(true),
        "stats.search_succeeded should be true"
    );
}
