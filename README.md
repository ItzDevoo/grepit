# grep4ai

AI-native code search — structured JSON output, relevance ranking, token budgets, and smart context windowing. Built in Rust for speed.

## Install

```bash
npm install -g @grep4ai/cli
```

Or build from source:

```bash
cargo install --path crates/core
```

## A/B Test Results

Tested with Claude Code on 9 real coding tasks (3 categories: bug fixes, refactors, greenfield features):

| Metric | ripgrep + manual | grep4ai | Change |
|---|---|---|---|
| Task completion time | baseline | -47% | Faster |
| Tool calls per task | baseline | -59% | Fewer calls |
| Code quality (blind review) | 4.1/5 | 4.2/5 | Same quality |

## Benchmark Table

All 9 benchmark runs on the same hardware (M2 MacBook Pro, 16GB RAM):

| Task | Category | ripgrep calls | grep4ai calls | Time saved |
|---|---|---|---|---|
| Fix auth bug | Bug fix | 12 | 4 | 52% |
| Fix race condition | Bug fix | 8 | 3 | 41% |
| Fix memory leak | Bug fix | 14 | 6 | 48% |
| Rename module | Refactor | 6 | 3 | 38% |
| Extract service | Refactor | 11 | 5 | 55% |
| Split monolith | Refactor | 18 | 7 | 61% |
| Add API endpoint | Greenfield | 9 | 4 | 44% |
| Build CLI tool | Greenfield | 7 | 3 | 43% |
| Create test suite | Greenfield | 10 | 4 | 50% |

**Methodology note:** Each task was run twice (once with ripgrep, once with grep4ai) with the same prompt. Order was randomized to control for caching effects. "Tool calls" counts all search-related tool invocations. Time measured from first tool call to final code edit. These numbers reflect a specific set of tasks and prompts — your results may vary depending on codebase size, query complexity, and agent configuration.

## Claude Code MCP Setup

```bash
claude mcp add grep4ai -- npx @grep4ai/mcp
```

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "grep4ai": {
      "command": "node",
      "args": ["/path/to/grep4ai/mcp/index.js"]
    }
  }
}
```

Or if installed globally via npm:

```json
{
  "mcpServers": {
    "grep4ai": {
      "command": "npx",
      "args": ["grep4ai-mcp"]
    }
  }
}
```

The MCP server exposes three tools:
- **`search`** — Full-featured pattern search with ranking, context, and token budgets
- **`find_definitions`** — Find function/class/struct definitions by symbol name
- **`ping`** — Health check that verifies the binary is reachable

## CLI Reference

```
grep4ai [OPTIONS] <PATTERN> [PATH...]
```

### Search Options

| Flag | Description |
|---|---|
| `-i, --ignore-case` | Case-insensitive search |
| `-w, --word` | Match whole words only |
| `-F, --fixed-strings` | Treat pattern as literal string, not regex |
| `-t, --type <TYPE>` | Only search files of this type (e.g., rust, python, js) |
| `-T, --type-not <TYPE>` | Exclude files of this type |
| `-g, --glob <GLOB>` | Include files matching glob pattern |
| `--no-ignore` | Don't respect .gitignore files |
| `--hidden` | Search hidden files and directories |
| `-m, --max-count <N>` | Maximum matches per file |
| `--max-filesize <SIZE>` | Skip files larger than this (e.g., 1M, 500K) |
| `--max-depth <N>` | Maximum directory traversal depth |

### Output Options

| Flag | Description |
|---|---|
| `-f, --format <FMT>` | Output format: json (default), jsonl, compact, human |
| `--pretty` | Pretty-print JSON output |
| `--no-stats` | Omit statistics from output |

### Context Options

| Flag | Description |
|---|---|
| `-C, --context <N>` | Lines of context around each match |
| `-A, --after <N>` | Lines of context after each match |
| `-B, --before <N>` | Lines of context before each match |
| `--merge-context` | Merge overlapping context regions in the same file |

### AI Agent Options

| Flag | Description |
|---|---|
| `--token-budget <N>` | Maximum tokens in output (heuristic estimation) |
| `--rank` | Enable relevance ranking |
| `--no-rank` | Disable relevance ranking |
| `--dedup` | Collapse near-duplicate results |
| `--max-results <N>` | Maximum results to return (default: 100) |
| `--explain` | Show signal breakdown for each result's ranking |

### Meta

| Flag | Description |
|---|---|
| `-j, --threads <N>` | Number of search threads (default: auto-detect) |
| `--debug` | Print debug/timing info to stderr |
| `--version` | Print version |

## JSON Output Schema

```json
{
  "results": [
    {
      "path": "src/auth/handler.rs",
      "line": 42,
      "column": 5,
      "match_text": "authenticate",
      "context": {
        "before": ["use crate::db;", ""],
        "match_line": "pub fn authenticate(user: &User) -> Result<Token> {",
        "after": ["    let db = db::connect()?;", "    let hash = user.password_hash();"]
      },
      "score": 0.95,
      "file_type": "rust",
      "explain": [
        "definition (fn/class/struct/type declaration)",
        "file path strongly matches query",
        "core source path (src/lib/core)"
      ]
    }
  ],
  "stats": {
    "search_succeeded": true,
    "total_matches": 47,
    "results_returned": 10,
    "files_searched": 234,
    "files_skipped": 12,
    "duration_ms": 23,
    "tokens_used": 1847,
    "token_budget": 2000,
    "truncated": true,
    "skipped_high_relevance_count": 2,
    "top_files": [
      { "path": "src/auth/handler.rs", "match_count": 8 }
    ]
  }
}
```

## Example: Ranked Search with Explain

```bash
$ grep4ai --explain --pretty "fn main" src/
```

```json
{
  "results": [
    {
      "path": "src/main.rs",
      "line": 13,
      "column": 1,
      "match_text": "fn main",
      "context": {
        "before": ["use clap::Parser;", ""],
        "match_line": "fn main() {",
        "after": ["    let args = cli::Args::parse();", ""]
      },
      "score": 0.97,
      "file_type": "rust",
      "explain": [
        "definition (fn/class/struct/type declaration)",
        "core source path (src/lib/core)",
        "exact word match",
        "near top of file (declaration zone)",
        "source code file"
      ]
    }
  ],
  "stats": {
    "search_succeeded": true,
    "total_matches": 1,
    "results_returned": 1,
    "files_searched": 12,
    "files_skipped": 0,
    "duration_ms": 3,
    "truncated": false
  }
}
```

## When to Use grep4ai vs ripgrep

**Use grep4ai when:**
- You're building AI agent tooling and need structured JSON output
- You want relevance-ranked results (definitions first, source over tests)
- You need token budget enforcement for LLM context windows
- You want one tool call instead of grep + parse + rank + truncate

**Use ripgrep when:**
- You're searching interactively in a terminal
- You need streaming output for very large result sets
- You need advanced regex features (backreferences, lookahead)
- You want maximum raw throughput with no ranking overhead
- You need PCRE2 support

ripgrep is an excellent tool — grep4ai builds on the same `ignore` crate for file traversal and complements it with AI-specific features rather than replacing it.

## Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-change`
3. Make your changes and add tests
4. Run the test suite: `cargo test --workspace`
5. Run clippy: `cargo clippy --workspace -- -D warnings`
6. Run formatter: `cargo fmt --check`
7. Submit a pull request

The codebase is organized as a Cargo workspace:

| Crate | Purpose |
|---|---|
| `crates/core` | CLI binary and pipeline orchestration |
| `crates/walker` | Parallel, gitignore-aware file discovery |
| `crates/searcher` | Memory-mapped parallel regex search |
| `crates/ranker` | Relevance scoring and deduplication |
| `crates/context` | Smart context window extraction |
| `crates/output` | JSON, JSONL, compact, and human formatters |
| `crates/tokens` | Token estimation and budget enforcement |
| `mcp/` | MCP server for Claude Code integration |

## License

MIT
