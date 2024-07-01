//! Rollup types relating to the consensus-layer state of the rollup.
//!
//! Types relating to the execution-layer state are kept generic, not
//! reusing any Reth types.

pub mod block;
pub mod block_template;
pub mod chain_state;
pub mod client_state;
pub mod l1;
pub mod operation;
pub mod state_op;
pub mod sync_event;
