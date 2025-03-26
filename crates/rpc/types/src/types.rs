//! Data structures for that represents the JSON responses. `rpc` crate should depend on this.
//!
//!  Following the <https://github.com/rust-bitcoin/rust-bitcoincore-rpc> where there are separate crates for
//!  - implementation of RPC client
//!  - crate for just data structures that represents the JSON responses from Bitcoin core RPC

use bitcoin::{Amount, BlockHash, Network, OutPoint as BitcoinOutPoint, TapNodeHash, Txid, Wtxid};
use serde::{Deserialize, Serialize};
use strata_db::types::{CheckpointConfStatus, CheckpointEntry};
use strata_primitives::{
    bitcoin_bosd::Descriptor,
    bridge::{BitcoinBlockHeight, OperatorIdx},
    epoch::EpochCommitment,
    l1::{BitcoinAddress, BitcoinAmount, L1BlockCommitment, OutputRef},
    l2::L2BlockCommitment,
    prelude::L1Status,
};
use strata_state::{
    batch::BatchInfo,
    bridge_ops::WithdrawalIntent,
    bridge_state::{DepositEntry, DepositState},
    client_state::CheckpointL1Ref,
    id::L2BlockId,
};

/// The various duties that can be assigned to an operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum BridgeDuty {
    /// The duty to create and sign a Deposit Transaction so as to move funds from the user to the
    /// Bridge Address.
    ///
    /// This duty is created when a user deposit request comes in, and applies to all operators.
    SignDeposit(DepositInfo),

    /// The duty to fulfill a withdrawal request that is assigned to a particular operator.
    ///
    /// This duty is created when a user requests a withdrawal by calling a precompile in the EL
    /// and the [`crate::bridge_state::DepositState`] transitions to
    /// [`crate::bridge_state::DepositState::Dispatched`].
    ///
    /// This kicks off the withdrawal process which involves cooperative signing by the operator
    /// set, or a more involved unilateral withdrawal process (in the future) if not all operators
    /// cooperate in the process.
    FulfillWithdrawal(CooperativeWithdrawalInfo),
}

/// The deposit information  required to create the Deposit Transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositInfo {
    /// The deposit request transaction outpoints from the users.
    pub deposit_request_outpoint: BitcoinOutPoint,

    /// The stake index that will be tied to this deposit.
    ///
    /// This is required in order to make sure that the at withdrawal time, deposit UTXOs are
    /// assigned in the same order that the stake transactions were linked during setup time
    ///
    /// # Note
    ///
    /// The stake index must be encoded in 4-byte big-endian.
    pub stake_index: u32,

    /// The execution layer address to mint the equivalent tokens to.
    /// As of now, this is just the 20-byte EVM address.
    pub el_address: Vec<u8>,

    /// The amount in bitcoins that the user is sending.
    ///
    /// This amount should be greater than the [`BRIDGE_DENOMINATION`] for the deposit to be
    /// confirmed on bitcoin. The excess amount is used as miner fees for the Deposit Transaction.
    pub total_amount: Amount,

    /// The hash of the take back leaf in the Deposit Request Transaction (DRT) as provided by the
    /// user in their `OP_RETURN` output.
    pub take_back_leaf_hash: TapNodeHash,

    /// The original taproot address in the Deposit Request Transaction (DRT) output used to
    /// sanity check computation internally i.e., whether the known information (n/n script spend
    /// path, [`static@UNSPENDABLE_INTERNAL_KEY`]) + the [`Self::take_back_leaf_hash`] yields the
    /// same P2TR address.
    pub original_taproot_addr: BitcoinAddress,
}

impl DepositInfo {
    /// Create a new deposit info with all the necessary data required to create a deposit
    /// transaction.
    pub fn new(
        deposit_request_outpoint: BitcoinOutPoint,
        stake_index: u32,
        el_address: Vec<u8>,
        total_amount: Amount,
        take_back_leaf_hash: TapNodeHash,
        original_taproot_addr: BitcoinAddress,
    ) -> Self {
        Self {
            deposit_request_outpoint,
            stake_index,
            el_address,
            total_amount,
            take_back_leaf_hash,
            original_taproot_addr,
        }
    }
}

impl From<DepositInfo> for BridgeDuty {
    fn from(value: DepositInfo) -> Self {
        Self::SignDeposit(value)
    }
}

/// Details for a withdrawal info assigned to an operator.
///
/// It has all the information required to create a transaction for fulfilling a user's withdrawal
/// request and pay operator fees.
// TODO: This can be multiple withdrawal destinations by adding
//       that `user_destination` is `IntoIterator<Descriptor>`
//       and the user can send a single BOSD or multiple BOSDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CooperativeWithdrawalInfo {
    /// The [`OutPoint`] of the UTXO in the Bridge Address that is to be used to service the
    /// withdrawal request.
    deposit_outpoint: BitcoinOutPoint,

    /// The BOSD [`Descriptor`] supplied by the user.
    user_destination: Descriptor,

    /// The index of the operator that is assigned the withdrawal.
    assigned_operator_idx: OperatorIdx,

    /// The bitcoin block height before which the withdrawal has to be processed.
    ///
    /// Any withdrawal request whose `exec_deadline` is before the current bitcoin block height is
    /// considered stale and must be ignored.
    exec_deadline: BitcoinBlockHeight,
}

impl CooperativeWithdrawalInfo {
    /// Create a new withdrawal request.
    pub fn new(
        deposit_outpoint: BitcoinOutPoint,
        user_destination: Descriptor,
        assigned_operator_idx: OperatorIdx,
        exec_deadline: BitcoinBlockHeight,
    ) -> Self {
        Self {
            deposit_outpoint,
            user_destination,
            assigned_operator_idx,
            exec_deadline,
        }
    }
}

impl From<CooperativeWithdrawalInfo> for BridgeDuty {
    fn from(value: CooperativeWithdrawalInfo) -> Self {
        Self::FulfillWithdrawal(value)
    }
}

/// The various states a bridge duty may be in.
///
/// The full state transition looks as follows:
///
/// `Received` --|`CollectingNonces`|--> `CollectedNonces` --|`CollectingPartialSigs`|-->
/// `CollectedSignatures` --|`Broadcasting`|--> `Executed`.
///
/// The duty execution might fail as well at any step in which case the status would be `Failed`.
///
/// # Note
///
/// This type does not dictate the exact state transition path. A transition from `Received` to
/// `Executed` is perfectly valid to allow for maximum flexibility.
// TODO: use a typestate pattern with a `next` method that does the state transition. This can
// be left as is to allow for flexible level of granularity. For example, one could just have
// `Received`, `CollectedSignatures` and `Executed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeDutyStatus {
    /// The duty has been received.
    ///
    /// This usually entails collecting nonces before the corresponding transaction can be
    /// partially signed.
    Received,

    /// The required nonces are being collected.
    CollectingNonces {
        /// The number of nonces collected so far.
        collected: u32,

        /// The indexes of operators that are yet to provide nonces.
        remaining: Vec<OperatorIdx>,
    },

    /// The required nonces have been collected.
    ///
    /// This state can be inferred from the previous state but might still be useful as the
    /// required number of nonces is context-driven and it cannot be determined whether all
    /// nonces have been collected by looking at the above variant alone.
    CollectedNonces,

    /// The partial signatures are being collected.
    CollectingSignatures {
        /// The number of nonces collected so far.
        collected: u32,

        /// The indexes of operators that are yet to provide partial signatures.
        remaining: Vec<OperatorIdx>,
    },

    /// The required partial signatures have been collected.
    ///
    /// This state can be inferred from the previous state but might still be useful as the
    /// required number of signatures is context-driven and it cannot be determined whether all
    /// partial signatures have been collected by looking at the above variant alone.
    CollectedSignatures,

    /// The duty has been executed.
    ///
    /// This means that the required transaction has been fully signed and broadcasted to Bitcoin.
    Executed,

    /// The duty could not be executed.
    ///
    /// Holds the error message as a [`String`] for context.
    // TODO: this should hold `strata-bridge-exec::ExecError` instead but that requires
    // implementing `BorshSerialize` and `BorshDeserialize`.
    Failed(String),
}

impl Default for BridgeDutyStatus {
    fn default() -> Self {
        Self::Received
    }
}

impl BridgeDutyStatus {
    /// Checks if the [`BridgeDutyStatus`] is in its final state.
    pub fn is_done(&self) -> bool {
        matches!(self, BridgeDutyStatus::Executed)
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
pub struct HexBytes64(#[serde(with = "hex::serde")] pub [u8; 64]);

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

/// In reference to checkpointed client state tracked by the CSM.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcClientStatus {
    /// Blockchain tip.
    // TODO remove this since the CSM doesn't track this anymore, we're pulling it in indirectly
    #[serde(with = "hex::serde")]
    #[deprecated(note = "no longer tracked by client state")]
    pub chain_tip: [u8; 32],

    /// L1 chain tip slot.
    // TODO remove this since the CSM doesn't track this anymore, we're pulling it in indirectly
    #[deprecated(note = "no longer tracked by client state")]
    pub chain_tip_slot: u64,

    /// L2 block that's been finalized and proven on L1.
    #[serde(with = "hex::serde")]
    #[deprecated(note = "implied by finalized_epoch, use that instead")]
    pub finalized_blkid: [u8; 32],

    /// Epoch that's been confirmed and buried on L1 and we can assume won't
    /// roll back.
    pub finalized_epoch: Option<EpochCommitment>,

    /// Epoch that's been confirmed on L1 but might still roll back.
    pub confirmed_epoch: Option<EpochCommitment>,

    /// Recent L1 block that we might still reorg.
    #[serde(with = "hex::serde")]
    #[deprecated(note = "use `tip_l1_block`")]
    pub last_l1_block: [u8; 32],

    /// L1 block index we treat as being "buried" and won't reorg.
    #[deprecated(note = "use `buried_l1_block`")]
    pub buried_l1_height: u64,

    /// Tip L1 block that we're following.
    pub tip_l1_block: Option<L1BlockCommitment>,

    /// Buried L1 block that we use to determine the finalized epoch.
    pub buried_l1_block: Option<L1BlockCommitment>,
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
    // TODO consolidate into using L2BlockCommitment
    pub tip_height: u64,

    /// Last L2 block we've chosen as the current tip.
    // TODO consolidate into using L2BlockCommitment
    pub tip_block_id: strata_state::id::L2BlockId,

    /// Current epoch from chainstate.
    pub cur_epoch: u64,

    /// Previous epoch from chainstate.
    pub prev_epoch: EpochCommitment,

    /// Observed finalized epoch from chainstate.
    pub observed_finalized_epoch: EpochCommitment,

    /// Most recent L1 block we've acted on on-chain.
    pub safe_l1_block: L1BlockCommitment,

    /// Terminal blkid of observed finalized epoch from chainstate.
    ///
    /// Note that this is not necessarily the most recently finalized epoch,
    /// it's the one we've also observed, so it's behind by >~1.
    ///
    /// If you want the real one from L1, use another method.
    // TODO which other method?
    #[deprecated]
    pub finalized_block_id: L2BlockId,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RawBlockWitness {
    pub raw_l2_block: Vec<u8>,
    pub raw_chain_state: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RpcCheckpointConfStatus {
    /// Pending to be posted on L1
    Pending,
    /// Confirmed on L1
    Confirmed,
    /// Finalized on L1
    Finalized,
}

impl From<CheckpointConfStatus> for RpcCheckpointConfStatus {
    fn from(value: CheckpointConfStatus) -> Self {
        match value {
            CheckpointConfStatus::Pending => Self::Pending,
            CheckpointConfStatus::Confirmed(_) => Self::Confirmed,
            CheckpointConfStatus::Finalized(_) => Self::Finalized,
        }
    }
}

impl From<CheckpointEntry> for RpcCheckpointConfStatus {
    fn from(value: CheckpointEntry) -> Self {
        value.confirmation_status.into()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcCheckpointInfo {
    /// The index of the checkpoint
    pub idx: u64,
    /// L1 range  the checkpoint covers
    pub l1_range: (L1BlockCommitment, L1BlockCommitment),
    /// L2 range the checkpoint covers
    pub l2_range: (L2BlockCommitment, L2BlockCommitment),
    /// Info on txn where checkpoint is committed on chain
    pub l1_reference: Option<RpcCheckpointL1Ref>,
    /// Confirmation status of checkpoint
    pub confirmation_status: RpcCheckpointConfStatus,
}

impl From<BatchInfo> for RpcCheckpointInfo {
    fn from(value: BatchInfo) -> Self {
        Self {
            idx: value.epoch,
            l1_range: value.l1_range,
            l2_range: value.l2_range,
            l1_reference: None,
            confirmation_status: RpcCheckpointConfStatus::Pending,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcCheckpointL1Ref {
    pub block_height: u64,
    pub block_id: BlockHash,
    pub txid: Txid,
    pub wtxid: Wtxid,
}

impl From<CheckpointL1Ref> for RpcCheckpointL1Ref {
    fn from(l1ref: CheckpointL1Ref) -> Self {
        Self {
            block_height: l1ref.l1_commitment.height(),
            block_id: (*l1ref.l1_commitment.blkid()).into(),
            txid: l1ref.txid.into(),
            wtxid: l1ref.wtxid.into(),
        }
    }
}

impl From<CheckpointEntry> for RpcCheckpointInfo {
    fn from(value: CheckpointEntry) -> Self {
        let mut item: Self = value.checkpoint.batch_info().clone().into();
        item.l1_reference = match value.confirmation_status.clone() {
            CheckpointConfStatus::Pending => None,
            CheckpointConfStatus::Confirmed(lref) => Some(lref.into()),
            CheckpointConfStatus::Finalized(lref) => Some(lref.into()),
        };
        item.confirmation_status = value.confirmation_status.into();
        item
    }
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

    /// Deposit state.
    state: DepositState,
}

impl RpcDepositEntry {
    pub fn from_deposit_entry(ent: &DepositEntry) -> Self {
        Self {
            deposit_idx: ent.idx(),
            output: *ent.output(),
            notary_operators: ent.notary_operators().to_vec(),
            amt: ent.amt(),
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
