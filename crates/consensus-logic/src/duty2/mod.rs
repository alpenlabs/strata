//! Reimplementing duties in a more structured way.
//!
//! This exists because I can't figure out how I would realistically refactor
//! the existing duty infrastructure.
//!
//! This will replace the existing `duty` module when finished.

pub mod duty_sign_block;
pub mod errors;
pub mod extractor;
pub mod types;
pub mod worker;
