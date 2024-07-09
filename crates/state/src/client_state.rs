//! Consensus types that track node behavior as we receive messages from the L1
//! chain and the p2p network.  These will be expanded further as we actually
//! implement the consensus logic.
// TODO move this to another crate that contains our sync logic

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{chain_state::ChainState, id::L2BlockId, l1::L1BlockId};

/// High level client's state of the network.  This is local to the client, not
/// coordinated as part of the L2 chain.
///
/// This is updated when we see a consensus-relevant message.  This is L2 blocks
/// but also L1 blocks being published with interesting things in them, and
/// various other events.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct ClientState {
    /// Blockchain tip.
    pub(super) chain_tip: L2BlockId,

    /// L2 block that's been finalized and proven on L1.
    pub(super) finalized_blkid: L2BlockId,

    /// Recent L1 blocks that we might still reorg.
    // TODO replace with a tracker that we can reorg
    pub(super) recent_l1_blocks: Vec<L1BlockId>,

    /// L1 block index we treat as being "buried" and won't reorg.
    pub(super) buried_l1_height: u64,
}

impl ClientState {
    pub fn from_genesis(genesis_chstate: &ChainState, genesis_l1_height: u64) -> Self {
        let gblkid = genesis_chstate.accepted_l2_blocks[0];
        Self {
            chain_tip: gblkid,
            finalized_blkid: gblkid,
            recent_l1_blocks: Vec::new(),
            buried_l1_height: genesis_l1_height,
        }
    }

    pub fn chain_tip_blkid(&self) -> &L2BlockId {
        &self.chain_tip
    }

    pub fn finalized_blkid(&self) -> &L2BlockId {
        &self.finalized_blkid
    }
}
