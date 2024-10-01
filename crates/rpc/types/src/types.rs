//! Data structures for that represents the JSON responses. `rpc` crate should depend on this.
//!
//!  Following the <https://github.com/rust-bitcoin/rust-bitcoincore-rpc> where there are separate crates for
//!  - implementation of RPC client
//!  - crate for just data structures that represents the JSON responses from Bitcoin core RPC

use alpen_express_state::{
    batch::CheckpointInfo, bridge_duties::BridgeDuty, bridge_ops::WithdrawalIntent, id::L2BlockId,
};
use bitcoin::{Network, Txid};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexBytes(#[serde(with = "hex::serde")] pub Vec<u8>);

impl HexBytes {
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for HexBytes {
    fn from(value: Vec<u8>) -> Self {
        HexBytes(value)
    }
}

impl From<&[u8]> for HexBytes {
    fn from(value: &[u8]) -> Self {
        HexBytes(value.to_vec())
    }
}

impl From<Box<[u8]>> for HexBytes {
    fn from(value: Box<[u8]>) -> Self {
        HexBytes(value.into_vec())
    }
}

impl From<HexBytes> for Vec<u8> {
    fn from(value: HexBytes) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexBytes32(#[serde(with = "hex::serde")] pub [u8; 32]);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Status {
    /// If the last time we tried to poll the client (as of `last_update`)
    /// we were successful.
    pub bitcoin_rpc_connected: bool,

    /// The last error message we received when trying to poll the client, if
    /// there was one.
    pub last_rpc_error: Option<String>,

    /// Current block height.
    pub cur_height: u64,

    /// Current tip block ID as string.
    pub cur_tip_blkid: String,

    /// Last published txid where L2 blob was present
    pub last_published_txid: Option<Txid>,

    /// number of published transactions in current run (commit + reveal pair count as 1)
    pub published_inscription_count: u64,

    /// UNIX millis time of the last time we got a new update from the L1 connector.
    pub last_update: u64,

    /// Underlying network.
    pub network: Network,
}

impl Default for L1Status {
    fn default() -> Self {
        Self {
            bitcoin_rpc_connected: Default::default(),
            last_rpc_error: Default::default(),
            cur_height: Default::default(),
            cur_tip_blkid: Default::default(),
            last_published_txid: Default::default(),
            published_inscription_count: Default::default(),
            last_update: Default::default(),
            network: Network::Regtest,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientStatus {
    /// Blockchain tip.
    #[serde(with = "hex::serde")]
    pub chain_tip: [u8; 32],

    /// L1 chain tip slot.
    pub chain_tip_slot: u64,

    /// L2 block that's been finalized and proven on L1.
    #[serde(with = "hex::serde")]
    pub finalized_blkid: [u8; 32],

    /// Recent L1 block that we might still reorg.
    #[serde(with = "hex::serde")]
    pub last_l1_block: [u8; 32],

    /// L1 block index we treat as being "buried" and won't reorg.
    pub buried_l1_height: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlockHeader {
    /// The index of the block representing height.
    pub block_idx: u64,

    /// The timestamp of when the block was created in UNIX epoch format.
    pub timestamp: u64,

    /// hash of the block's contents.
    #[serde(with = "hex::serde")]
    pub block_id: [u8; 32],

    /// previous block
    #[serde(with = "hex::serde")]
    pub prev_block: [u8; 32],

    // L1 segment hash
    #[serde(with = "hex::serde")]
    pub l1_segment_hash: [u8; 32],

    /// Hash of the execution segment
    #[serde(with = "hex::serde")]
    pub exec_segment_hash: [u8; 32],

    /// The root hash of the state tree
    #[serde(with = "hex::serde")]
    pub state_root: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DaBlob {
    /// The destination or identifier for the blob.
    pub dest: u8,

    ///  The commitment hash for blob
    pub blob_commitment: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecUpdate {
    /// The index of the update, used to track or sequence updates.
    pub update_idx: u64,

    /// Merkle tree root of the contents of the EL payload, in the order it was
    /// expressed in the block.
    #[serde(with = "hex::serde")]
    pub entries_root: [u8; 32],

    /// Buffer of any other payload data.  This is used with the other fields
    /// here to construct the full EVM header payload.
    #[serde(with = "hex::serde")]
    pub extra_payload: Vec<u8>,

    /// New state root for the update.  This is not just the inner EL payload,
    /// but also any extra bookkeeping we need across multiple.
    #[serde(with = "hex::serde")]
    pub new_state: [u8; 32],

    /// Bridge withdrawal intents.
    pub withdrawals: Vec<WithdrawalIntent>,

    /// DA blobs that we expect to see on L1.  This may be empty, probably is
    /// only set near the end of the range of blocks in a batch since we only
    /// assert these in a per-batch frequency.
    pub da_blobs: Vec<DaBlob>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeSyncStatus {
    /// Current head L2 slot known to this node
    pub tip_height: u64,

    /// Last L2 block we've chosen as the current tip.
    pub tip_block_id: alpen_express_state::id::L2BlockId,

    /// L2 block that's been finalized and proven on L1.
    pub finalized_block_id: L2BlockId,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RawBlockWitness {
    pub raw_l2_block: Vec<u8>,
    pub raw_chain_state: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcCheckpointInfo {
    /// The index of the checkpoint
    pub idx: u64,
    /// L1 height  the checkpoint covers
    pub l1_height: u64,
    /// L2 height the checkpoint covers
    pub l2_height: u64,
    /// L2 block that this checkpoint covers
    pub l2_blockid: L2BlockId,
}

impl From<CheckpointInfo> for RpcCheckpointInfo {
    fn from(value: CheckpointInfo) -> Self {
        Self {
            idx: value.idx,
            l1_height: value.l1_range.1,
            l2_height: value.l2_range.1,
            l2_blockid: value.l2_blockid,
        }
    }
}

/// The duties assigned to an operator within a given range.
///
/// # Note
///
/// The `index`'s are only relevant for Deposit duties as those are stored off-chain in a database.
/// The withdrawal duties are fetched from the current chain state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDuties {
    /// The actual [`BridgeDuty`]'s assigned to an operator which includes both the deposit and
    /// withdrawal duties.
    pub duties: Vec<BridgeDuty>,

    /// The starting index (inclusive) from which the duties are fetched.
    pub start_index: u64,

    /// The last block index (inclusive) upto which the duties are feched.
    pub stop_index: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum L2BlockFinalizationStatus {
    Unfinalized,
    Pending,
    Finalized
}
