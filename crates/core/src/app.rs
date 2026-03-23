//! Application logic — wires all modules together into the search pipeline.

use anyhow::Result;
use std::path::PathBuf;
use std::time::Instant;

use grep4ai_context::{extract_context, merge_overlapping, ContextConfig};
use grep4ai_output::{write_output, OutputConfig, OutputFormat};
use grep4ai_ranker::dedup::DedupConfig;
use grep4ai_ranker::{deduplicate, rank_matches, RankConfig};
use grep4ai_searcher::{SearchConfig, SearchEngine};
use grep4ai_walker::{Walker, WalkerConfig};

use crate::cli::{parse_filesize, Args};

/// Run the full search pipeline with the given CLI arguments.
/// Returns the number of matches found (for exit code purposes).
pub fn run(args: Args) -> Result<u64> {
    let start = Instant::now();

    // ── 1. Configure walker and search engine ────────────────────────
    let walker_config = WalkerConfig {
        paths: args.paths.iter().map(PathBuf::from).collect(),
        threads: args.threads.unwrap_or(0),
        respect_gitignore: !args.no_ignore,
        search_hidden: args.hidden,
        max_depth: args.max_depth,
        max_filesize: args.max_filesize.as_deref().and_then(parse_filesize),
        globs: args.glob.clone(),
        include_types: args.file_type.clone(),
        exclude_types: args.type_not.clone(),
    };

    let search_config = SearchConfig {
        pattern: args.pattern.clone(),
        ignore_case: args.ignore_case,
        word_boundary: args.word,
        fixed_string: args.fixed_strings,
        max_count_per_file: args.max_count,
    };

    let engine = SearchEngine::new(search_config)?;
    let walker = Walker::new(walker_config);

    // ── 2. Pipeline: walk and search concurrently ────────────────────
    // The walker sends files into a channel while the searcher
    // consumes them in parallel — no waiting for the full file list.
    let (tx, rx) = walker.walk_channel();
    let (raw_matches, search_stats) = std::thread::scope(|s| {
        // Spawn walker in background — it feeds files into the channel
        s.spawn(move || {
            walker.start_walk(tx);
        });

        // Search files as they arrive from the walker
        engine.search_streaming(rx)
    });

    if args.debug {
        eprintln!(
            "[grep4ai] found {} matches in {} files ({}ms)",
            search_stats.total_matches,
            search_stats.files_searched,
            start.elapsed().as_millis()
        );
    }

    // ── 3. Rank results ─────────────────────────────────────────────
    let output_format: OutputFormat = if args.files_only {
        OutputFormat::FilesOnly
    } else if args.count {
        OutputFormat::Count
    } else {
        args.format
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))?
    };
    let should_rank = args.rank || (!args.no_rank && output_format == OutputFormat::Json);

    // For files-only and count modes, don't truncate matches in the ranker —
    // we need all matches to collect unique file paths, then limit in the output layer.
    let ranker_max = match output_format {
        OutputFormat::FilesOnly | OutputFormat::Count => None,
        _ => Some(args.max_results),
    };

    let rank_config = RankConfig {
        enabled: should_rank,
        max_results: ranker_max,
        query: args.pattern.clone(),
    };

    let scored_matches = rank_matches(raw_matches, &rank_config);

    // ── 4. Deduplicate if requested ─────────────────────────────────
    let scored_matches = if args.dedup {
        let dedup_config = DedupConfig {
            threshold: args.dedup_threshold,
        };
        let result = deduplicate(scored_matches, &dedup_config);
        if args.debug {
            eprintln!(
                "[grep4ai] collapsed {} duplicate matches",
                result.collapsed_count
            );
        }
        result.matches
    } else {
        scored_matches
    };

    // ── 5. Extract context ──────────────────────────────────────────
    let (before, after) = if let Some(c) = args.context {
        (c, c)
    } else {
        (
            args.before_context.unwrap_or(2),
            args.after_context.unwrap_or(2),
        )
    };

    let context_config = ContextConfig { before, after };
    let contextual_matches = extract_context(scored_matches, &context_config);

    // Merge overlapping context if requested
    let contextual_matches = if args.merge_context {
        merge_overlapping(contextual_matches)
    } else {
        contextual_matches
    };

    // ── 6. Format and write output ──────────────────────────────────
    let duration_ms = start.elapsed().as_millis() as u64;

    let output_config = OutputConfig {
        format: output_format,
        pretty: args.pretty,
        show_stats: !args.no_stats,
        token_budget: args.token_budget,
        explain: args.explain,
        max_results: Some(args.max_results),
    };

    let mut stdout = std::io::stdout().lock();
    write_output(
        &mut stdout,
        contextual_matches,
        search_stats.files_searched,
        search_stats.files_skipped,
        search_stats.total_matches,
        duration_ms,
        &output_config,
    )?;

    Ok(search_stats.total_matches)
}
