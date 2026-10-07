//! Executors that drive a pipeline of processor stages across a chain graph.
//!
//! An executor is told which links to traverse by an external sync engine.  For
//! each one it runs the registered [`GChainProc`](strata_gchain_types::GChainProc)
//! stages, holds onto the artifacts they produce, and later commits or rolls
//! back whole paths of them.

#![expect(
    missing_debug_implementations,
    reason = "executors hold stage trait objects and stores that don't implement Debug"
)]

mod artifact_cache;
mod config;
mod context;
mod core;
mod errors;
mod mem_store;
mod open;
mod reprocess;
mod serial_executor;
mod serial_ops;
mod store;
#[cfg(test)]
mod test_support;
mod tracking;

pub use artifact_cache::*;
pub use config::SerialExecutorBuilder;
pub use context::*;
pub use errors::*;
pub use mem_store::*;
pub use open::{OpenReport, OpenResult, ReprocExecutor};
pub use reprocess::{RejectedCommit, ReprocLog, StagePlan};
pub use serial_executor::*;
pub use serial_ops::LinkOutcome;
pub use store::*;
pub use tracking::*;
