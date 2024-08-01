#![feature(btree_extract_if)] // remove when we remove the stubs

//! Database abstraction layers, building on what Reth supports.

pub mod bridge;
pub mod database;
pub mod errors;
pub mod traits;
pub mod types;

#[cfg(feature = "stubs")]
pub mod stubs;

pub type DbResult<T> = anyhow::Result<T, errors::DbError>;
