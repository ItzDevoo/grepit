//! grepit — The fastest grep tool built for AI agents.
//!
//! Usage: grepit [OPTIONS] <PATTERN> [PATH...]
//!
//! AI-native search with structured JSON output, relevance ranking,
//! token budget awareness, and smart context windowing.

mod app;
mod cli;

use clap::Parser;

fn main() {
    let args = cli::Args::parse();

    if let Err(e) = app::run(args) {
        eprintln!("grepit: {e}");
        std::process::exit(1);
    }
}
