//! Interfaces to expose the context in which a block is being validated.

use strata_state::{header::L2BlockHeader, id::L2BlockId};

/// Provider for context about the block in the chain.
///
/// Does NOT provide access to chainstate information.  This is primarily
/// involving block headers.  It will probably also provide L1 manifests.
pub trait BlockContext {
    /// Returns the slot that we're checking.
    fn slot(&self) -> u64;

    /// Returns the unix millis timestamp of the block.
    fn timestamp(&self) -> u64;

    /// Returns the parent block's ID.
    fn parent_blkid(&self) -> &L2BlockId;

    /// Returns the parent block's header.
    fn parent_header(&self) -> &L2BlockHeader;
}
