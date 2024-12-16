//! Input-output with Bitcoin, implementing L1 chain trait.
#![allow(dead_code)] // TODO: remove this once `get_height_blkid` and `deepest_block` are used.

pub mod broadcaster;
pub mod reader;
pub mod rpc_client;
pub mod status;

#[cfg(feature = "test_utils")]
pub mod test_utils;
pub mod writer;
