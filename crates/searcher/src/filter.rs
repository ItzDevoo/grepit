//! Path-based filtering for noisy directories and files.

use std::path::Path;

/// Directories that typically contain generated/vendored/noisy content.
const NOISY_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    "dist",
    "build",
    "target",
    "__pycache__",
    ".git",
    ".svn",
    ".hg",
    "coverage",
    ".next",
    ".nuxt",
    "bower_components",
    ".cache",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    "venv",
    ".venv",
    "env",
    ".env",
];

/// File extensions that are typically not useful to search.
const NOISY_EXTENSIONS: &[&str] = &[
    "min.js", "min.css", "map", "lock", "sum", "pack", "wasm", "pyc", "pyo", "class", "o", "obj",
    "so", "dylib", "dll", "exe", "bin",
];

/// Returns true if the path should be skipped based on heuristics.
pub fn should_skip_path(path: &Path) -> bool {
    // Check if any path component is a noisy directory
    for component in path.components() {
        let name = component.as_os_str().to_str().unwrap_or("");
        if NOISY_DIRS.contains(&name) {
            return true;
        }
    }

    // Check noisy extensions
    let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    for ext in NOISY_EXTENSIONS {
        if filename.ends_with(ext) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_skip_node_modules() {
        assert!(should_skip_path(&PathBuf::from(
            "project/node_modules/foo/index.js"
        )));
    }

    #[test]
    fn test_skip_minified() {
        assert!(should_skip_path(&PathBuf::from("dist/bundle.min.js")));
    }

    #[test]
    fn test_allow_normal_files() {
        assert!(!should_skip_path(&PathBuf::from("src/main.rs")));
    }
}
