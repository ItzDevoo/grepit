//! Relevance scoring and deduplication for grep4ai search results.
//!
//! Ranks matches by combining multiple signals to surface the most
//! relevant results first — definitions over usages, source over tests.

pub mod dedup;
mod scorer;
mod signals;

pub use dedup::deduplicate;
pub use scorer::{rank_matches, RankConfig, ScoredMatch};
pub use signals::SignalSet;
