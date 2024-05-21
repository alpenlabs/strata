//! Database abstraction layers, building on what Reth supports.

pub mod errors;
pub mod l1;
pub mod macros;
pub mod traits;

pub type DbResult<T> = anyhow::Result<T, crate::errors::DbError>;
