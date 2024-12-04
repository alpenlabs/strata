//! Key derivation for Strata
//!
//! This crate contains the key derivation logic for Strata.
//!
//! It is split into two modules:
//! - `operator`: Key derivation for bridge operators
//! - `sequencer`: Key derivation for sequencer

pub mod error;
pub mod operator;
pub mod sequencer;
