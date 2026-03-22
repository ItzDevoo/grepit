//! Application logic — wires all modules together into the search pipeline.

use anyhow::Result;
use std::path::PathBuf;
use std::time::Instant;

use grepit_context::{extract_context, merge_overlapping, ContextConfig};
use grepit_output::{write_output, OutputConfig, OutputFormat};
use grepit_ranker::dedup::DedupConfig;
use grepit_ranker::{deduplicate, rank_matches, RankConfig};
use grepit_searcher::{SearchConfig, SearchEngine};
use grepit_walker::{Walker, WalkerConfig};

use crate::cli::{parse_filesize, Args};

/// Run the full search pipeline with the given CLI arguments.
pub fn run(args: Args) -> Result<()> {
    let start = Instant::now();

    // ── 1. Configure and run the walker ─────────────────────────────
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

    let walker = Walker::new(walker_config);
    let files = walker.collect_files();

    if args.debug {
        eprintln!("[grep4ai] discovered {} files", files.len());
    }

    // ── 2. Configure and run the search engine ──────────────────────
    let search_config = SearchConfig {
        pattern: args.pattern.clone(),
        ignore_case: args.ignore_case,
        word_boundary: args.word,
        fixed_string: args.fixed_strings,
        max_count_per_file: args.max_count,
    };

    let engine = SearchEngine::new(search_config)?;
    let (raw_matches, search_stats) = engine.search(&files);

    if args.debug {
        eprintln!(
            "[grep4ai] found {} matches in {} files ({}ms)",
            search_stats.total_matches,
            search_stats.files_searched,
            start.elapsed().as_millis()
        );
    }

    // ── 3. Rank results ─────────────────────────────────────────────
    let output_format: OutputFormat = args
        .format
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;
    let should_rank = args.rank || (!args.no_rank && output_format == OutputFormat::Json);

    let rank_config = RankConfig {
        enabled: should_rank,
        max_results: Some(args.max_results),
    };

    let scored_matches = rank_matches(raw_matches, &rank_config);

    // ── 4. Deduplicate if requested ─────────────────────────────────
    let scored_matches = if args.dedup {
        let dedup_config = DedupConfig::default();
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

    Ok(())
}
