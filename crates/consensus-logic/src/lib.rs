#![allow(dead_code)] // TODO: remove this once `finalized_tip` fn is used in `ForkChoiceManager`.
//! Consensus validation logic and core state machine

pub mod checkpoint;
pub mod client_transition;
pub mod ctl;
pub mod duty;
pub mod fork_choice_manager;
pub mod genesis;
pub mod l1_handler;
pub mod message;
pub mod precondition;
pub mod reorg;
pub mod state_tracker;
pub mod sync_manager;
pub mod unfinalized_tracker;
pub mod worker;

pub mod errors;
