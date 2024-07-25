#![feature(btree_extract_if)] // remove when we remove the stubs

pub mod database;
pub mod errors;
pub mod stubs;
pub mod traits;
pub mod types;

pub type DbResult<T> = anyhow::Result<T, errors::DbError>;
