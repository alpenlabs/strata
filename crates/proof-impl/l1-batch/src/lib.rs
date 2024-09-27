//! This crate implements the aggregation of consecutive L1 blocks to form a single proof

pub mod header_verification;
pub mod logic;
#[cfg(any(test, feature = "test-utils"))]
pub mod mock;
pub mod params;
pub mod timestamp_store;
