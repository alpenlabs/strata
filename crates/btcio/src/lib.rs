//! Input-output with Bitcoin, implementing L1 chain trait.
#![allow(dead_code)] // TODO: remove this once `get_height_blkid` and `deepest_block` are used.

pub mod broadcaster;
pub mod parser;
pub mod reader;
pub mod rpc;
pub mod status;
#[cfg(test)]
pub(crate) mod test_utils;
pub mod writer;
