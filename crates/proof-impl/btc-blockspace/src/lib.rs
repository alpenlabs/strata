//! This crate implements the scanning an L1 block to extract all the relevant transactions. It
//! ensures censorship resistance and correct ordering of these transactions
pub mod block;
pub mod logic;
pub mod merkle;
pub mod prover;
pub mod tx;
mod tx_indexer;
