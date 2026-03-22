//! Token counting and budget enforcement for grepit.
//!
//! Ensures search output fits within an AI agent's context window
//! by counting tokens and truncating results when needed.

mod budget;
mod counter;
mod estimator;

pub use budget::{BudgetEnforcer, BudgetResult};
pub use counter::TokenCounter;
pub use estimator::estimate_tokens;
