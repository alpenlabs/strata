//! Data structures for that represents the JSON responses. `rpc` crate should depend on this.
//!
//!  Following the https://github.com/rust-bitcoin/rust-bitcoincore-rpc where there are separate crates for
//!  - implementation of RPC client
//!  - crate for just data structures that represents the JSON responses from Bitcoin core RPC

use alpen_express_state::{bridge_ops::WithdrawalIntent, id::L2BlockId};
use bitcoin::Txid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexBytes(#[serde(with = "hex::serde")] pub Vec<u8>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HexBytes32(#[serde(with = "hex::serde")] pub [u8; 32]);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepositState {
    Created,
    Accepted,
    Dispatched,
    Executed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DepositEntry {
    /// The index of the deposit, used to identify or track the deposit within the system.
    pub deposit_idx: u32,

    /// The amount of currency deposited.
    pub amt: u64,

    /// Deposit state.
    pub state: DepositState,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeSyncStatus {
    /// Current head L2 slot known to this node
    pub tip_height: u64,

    /// Last L2 block we've chosen as the current tip.
    pub tip_block_id: L2BlockId,

    /// L2 block that's been finalized and proven on L1.
    pub finalized_block_id: L2BlockId,
}
