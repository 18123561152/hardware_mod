//! Stable machine-level contracts for BurnCloud Node hardware inspection.
//!
//! NOTE: These contract definitions must be kept in sync with
//! burncloud repository: crates/platform/node/src/contracts.rs
//! Any divergence must be resolved in favor of the burncloud repository.

pub mod hardware;
pub mod metrics;

pub use hardware::*;
pub use metrics::*;
