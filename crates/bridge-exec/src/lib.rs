//! Executor for bridge duties.
//!
//! Provides methods that allow spawning of async threads that handle the deposit and withdrawal
//! operations as well as the ability to query their status.

pub mod config;
pub mod errors;
pub mod handler;
pub mod ws_client;
