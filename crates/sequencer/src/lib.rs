//! Sequencer duty module handles block assembly and checkpoint management.

pub mod block_assembly;
pub mod block_template;
pub(crate) mod checkpoint;
pub mod checkpoint_handle;
pub mod errors;
pub mod extractor;
pub mod types;
pub mod worker;
