#![allow(dead_code)] // TODO: remove this once `finalized_tip` fn is used in `ForkChoiceManager`.
//! Consensus validation logic and core state machine

pub mod csm;
pub mod fork_choice_manager;
pub mod genesis;
pub mod reorg;
pub mod sync_manager;
pub mod unfinalized_tracker;
pub mod util;

pub mod errors;
