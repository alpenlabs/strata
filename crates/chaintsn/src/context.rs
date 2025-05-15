//! Interfaces to expose the context in which a block is being validated.

use strata_primitives::prelude::*;
use strata_state::{
    chain_state::Chainstate,
    header::{L2BlockHeader, L2Header},
    id::L2BlockId,
};

use crate::errors::{ProviderError, ProviderResult};

/// Provider for context about the block in the chain.
///
/// Does NOT provide access to chainstate information.  This is primarily
/// involving block headers.  It will probably also provide L1 manifests.
pub trait BlockHeaderContext {
    /// Returns the slot that we're checking.
    fn slot(&self) -> u64;

    /// Returns the unix millis timestamp of the block.
    fn timestamp(&self) -> u64;

    /// Returns the parent block's ID.
    fn parent_blkid(&self) -> &L2BlockId;

    /// Returns the parent block's header.
    fn parent_header(&self) -> &L2BlockHeader;
}

#[derive(Clone, Eq, PartialEq)]
pub struct L2HeaderAndParent {
    header: L2BlockHeader,
    parent_blkid: L2BlockId,
    parent: L2BlockHeader,
}

impl L2HeaderAndParent {
    pub fn new(header: L2BlockHeader, parent_blkid: L2BlockId, parent: L2BlockHeader) -> Self {
        Self {
            header,
            parent_blkid,
            parent,
        }
    }

    pub fn new_simple(header: L2BlockHeader, parent: L2BlockHeader) -> Self {
        let parent_blkid = parent.get_blockid();
        Self {
            header,
            parent_blkid,
            parent,
        }
    }
}

impl BlockHeaderContext for L2HeaderAndParent {
    fn slot(&self) -> u64 {
        self.header.slot()
    }

    fn timestamp(&self) -> u64 {
        self.header.timestamp()
    }

    fn parent_blkid(&self) -> &L2BlockId {
        &self.parent_blkid
    }

    fn parent_header(&self) -> &L2BlockHeader {
        &self.parent
    }
}

/// Accessor for fetch and manipulate the state we're building on top of.
///
/// This is supersceding the `StateCache` type.
pub trait StateAccessor {
    /// Gets a ref to the state.
    ///
    /// This is a transitional accessor that we will deprecate and remove soon.
    fn state_untracked(&self) -> &Chainstate;

    /// Gets a mut ref to the state.
    ///
    /// This is a transitional accessor that we will deprecate and remove soon.
    fn state_mut_untracked(&mut self) -> &mut Chainstate;

    // Accessors for toplevel state fields.

    /// Gets the current slot.
    fn slot(&self) -> u64;

    /// Sets the current slot.
    fn set_slot(&mut self, slot: u64);

    /// Gets the previous block commitment.
    fn prev_block(&self) -> L2BlockCommitment;

    /// Sets the previous block commitment.
    fn set_prev_block(&mut self, block: L2BlockCommitment);

    /// Gets the current epoch.
    fn cur_epoch(&self) -> u64;

    /// Sets the current epoch index.
    fn set_cur_epoch(&mut self, epoch: u64);

    /// Gets the previous epoch.
    fn prev_epoch(&self) -> EpochCommitment;

    /// Sets the previous epoch.
    fn set_prev_epoch(&mut self, epoch: EpochCommitment);

    /// Gets the finalized epoch commitment.
    fn finalized_epoch(&self) -> EpochCommitment;

    /// Sets the finalized epoch commitment.
    fn set_finalized_epoch(&mut self, epoch: EpochCommitment);

    /// Gets the last L1 block commitment.
    fn last_l1_block(&self) -> L1BlockCommitment;

    /// Sets the safe L1 block.
    fn set_last_l1_block(&mut self, block: L1BlockCommitment);

    /// Gets the epoch finishing flag.
    fn epoch_finishing_flag(&self) -> bool;

    /// Sets the epoch finishing flag.
    fn set_epoch_finishing_flag(&mut self, flag: bool);

    // Accessors for ledger account entries.
    // TODO
}

/// Provider for queries to sideloaded state like L1 block manifests.
pub trait AuxProvider {
    /// Returns the height of the new tip.
    fn get_l1_tip_height(&self) -> u64;

    /// Fetches an L1 block manifest by height.
    fn get_l1_block_manifest(&self, height: u64) -> ProviderResult<L1BlockManifest>;
}
