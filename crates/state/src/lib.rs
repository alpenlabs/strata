#![allow(dead_code)] // TODO: remove once the bridge state `sanity_check` fn is used.
#![feature(is_sorted)] // TODO: switch to using crate

//! Rollup types relating to the consensus-layer state of the rollup.
//!
//! Types relating to the execution-layer state are kept generic, not
//! reusing any Reth types.

pub mod block;
pub mod header;
pub mod bridge_ops;
pub mod bridge_state;
pub mod chain_state;
pub mod client_state;
pub mod da_blob;
pub mod exec_env;
pub mod exec_update;
pub mod forced_inclusion;
pub mod id;
pub mod l1;
pub mod operation;
pub mod state_op;
pub mod state_queue;
pub mod sync_event;

pub mod prelude;
