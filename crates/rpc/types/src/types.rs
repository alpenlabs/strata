//! Data structures for that represents the JSON responses. `rpc` crate should depend on this.
//!
//!  Following the <https://github.com/rust-bitcoin/rust-bitcoincore-rpc> where there are separate crates for
//!  - implementation of RPC client
//!  - crate for just data structures that represents the JSON responses from Bitcoin core RPC

use bitcoin::{hashes::Hash, Network, Txid, Wtxid};
use serde::{Deserialize, Serialize};
use strata_db::types::{CheckpointCommitment, CheckpointEntry};
use strata_primitives::{
    bridge::OperatorIdx,
    l1::{BitcoinAmount, L1TxRef, OutputRef},
    prelude::L1Status,
};
use strata_state::{
    batch::BatchInfo,
    bridge_duties::BridgeDuty,
    bridge_ops::WithdrawalIntent,
    bridge_state::{DepositEntry, DepositState},
    id::L2BlockId,
    l1::L1BlockId,
};

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

impl From<&L2BlockId> for HexBytes32 {
    fn from(value: &L2BlockId) -> Self {
        Self(*value.as_ref())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcL1Status {
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
    pub published_envelope_count: u64,

    /// UNIX millis time of the last time we got a new update from the L1 connector.
    pub last_update: u64,

    /// Underlying network.
    pub network: Network,
}

impl RpcL1Status {
    pub fn from_l1_status(l1s: L1Status, network: Network) -> Self {
        Self {
            bitcoin_rpc_connected: l1s.bitcoin_rpc_connected,
            last_rpc_error: l1s.last_rpc_error,
            cur_height: l1s.cur_height,
            cur_tip_blkid: l1s.cur_tip_blkid,
            last_published_txid: l1s.last_published_txid.map(Into::into),
            published_envelope_count: l1s.published_reveal_txs_count,
            last_update: l1s.last_update,
            network,
        }
    }
}

impl Default for RpcL1Status {
    fn default() -> Self {
        Self {
            bitcoin_rpc_connected: Default::default(),
            last_rpc_error: Default::default(),
            cur_height: Default::default(),
            cur_tip_blkid: Default::default(),
            last_published_txid: Default::default(),
            published_envelope_count: Default::default(),
            last_update: Default::default(),
            network: Network::Regtest,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcClientStatus {
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
pub struct RpcBlockHeader {
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
pub struct RpcExecUpdate {
    /// The index of the update, used to track or sequence updates.
    pub update_idx: u64,

    /// Merkle tree root of the contents of the EL payload, in the order it was
    /// strataed in the block.
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
pub struct RpcSyncStatus {
    /// Current head L2 slot known to this node
    pub tip_height: u64,

    /// Last L2 block we've chosen as the current tip.
    pub tip_block_id: strata_state::id::L2BlockId,

    /// L2 block that's been finalized and proven on L1.
    pub finalized_block_id: L2BlockId,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RawBlockWitness {
    pub raw_l2_block: Vec<u8>,
    pub raw_chain_state: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcCheckpointCommitmentInfo {
    /// block where checkpoint was posted
    pub blockhash: L1BlockId,

    /// txid of txn for this checkpoint
    pub txid: Txid,

    /// wtxid of txn for this checkpoint
    pub wtxid: Wtxid,

    /// The height of the block where the checkpoint was posted.
    pub height: u64,

    /// The position of the checkpoint in the block.
    pub position: u32,
}

impl From<CheckpointCommitment> for RpcCheckpointCommitmentInfo {
    fn from(value: CheckpointCommitment) -> Self {
        Self {
            blockhash: value.blockhash.into(),
            txid: Txid::from_byte_array(*value.txid.as_ref()),
            wtxid: Wtxid::from_byte_array(*value.wtxid.as_ref()),
            height: value.block_height,
            position: value.position,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcCheckpointInfo {
    /// The index of the checkpoint
    pub idx: u64,
    /// L1 height  the checkpoint covers
    pub l1_range: (u64, u64),
    /// L2 height the checkpoint covers
    pub l2_range: (u64, u64),
    /// L2 block that this checkpoint covers
    pub l2_blockid: L2BlockId,
    /// Info on txn where checkpoint is committed on chain
    pub commitment: Option<RpcCheckpointCommitmentInfo>,
}

impl From<BatchInfo> for RpcCheckpointInfo {
    fn from(value: BatchInfo) -> Self {
        Self {
            idx: value.idx,
            l1_range: value.l1_range,
            l2_range: value.l2_range,
            l2_blockid: value.l2_blockid,
            commitment: None,
        }
    }
}

impl From<CheckpointEntry> for RpcCheckpointInfo {
    fn from(value: CheckpointEntry) -> Self {
        let mut item: Self = value.batch_info.into();
        item.commitment = value.commitment.map(Into::into);
        item
    }
}

/// The duties assigned to an operator within a given range.
///
/// # Note
///
/// The `index`'s are only relevant for Deposit duties as those are stored off-chain in a database.
/// The withdrawal duties are fetched from the current chain state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcBridgeDuties {
    /// The actual [`BridgeDuty`]'s assigned to an operator which includes both the deposit and
    /// withdrawal duties.
    pub duties: Vec<BridgeDuty>,

    /// The starting index (inclusive) from which the duties are fetched.
    pub start_index: u64,

    /// The last block index (inclusive) upto which the duties are feched.
    pub stop_index: u64,
}

/// Deposit entry for RPC corresponding to [`DepositEntry`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcDepositEntry {
    deposit_idx: u32,

    /// The outpoint that this deposit entry references.
    output: OutputRef,

    /// List of notary operators, by their indexes.
    // TODO convert this to a windowed bitmap or something
    notary_operators: Vec<OperatorIdx>,

    /// Deposit amount, in the native asset.
    amt: BitcoinAmount,

    /// Refs to txs in the maturation queue that will update the deposit entry
    /// when they mature.  This is here so that we don't have to scan a
    /// potentially very large set of pending transactions to reason about the
    /// state of the deposits.  This must be kept in sync when we do things
    /// though.
    // TODO probably removing this actually
    pending_update_txs: Vec<L1TxRef>,

    /// Deposit state.
    state: DepositState,
}

impl RpcDepositEntry {
    pub fn from_deposit_entry(ent: &DepositEntry) -> Self {
        Self {
            deposit_idx: ent.idx(),
            output: ent.output().clone(),
            notary_operators: ent.notary_operators().to_vec(),
            amt: ent.amt(),
            pending_update_txs: ent.pending_update_txs().to_vec(),
            state: ent.deposit_state().clone(),
        }
    }
}

/// status of L2 Block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum L2BlockStatus {
    /// Unknown block height
    Unknown,
    /// Block is received and present in the longest chain
    Confirmed,
    /// Block is now conformed on L1, and present at certain L1 height
    Verified(u64),
    /// Block is now finalized, certain depth has been reached in L1
    Finalized(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcChainState {
    /// Most recent seen block.
    pub tip_blkid: L2BlockId,

    /// The slot of the last produced block.
    pub tip_slot: u64,

    pub cur_epoch: u64,
}
