//! Macro trait def for the `strata_` RPC namespace using jsonrpsee.
use bitcoin::Txid;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use strata_common::{Action, WorkerType};
use strata_db::types::{L1TxEntry, L1TxStatus};
use strata_primitives::{batch::EpochSummary, bridge::PublickeyTable, epoch::EpochCommitment};
use strata_rpc_types::{
    types::{RpcBlockHeader, RpcClientStatus, RpcL1Status},
    HexBytes, HexBytes32, HexBytes64, L2BlockStatus, RpcChainState, RpcCheckpointConfStatus,
    RpcCheckpointInfo, RpcDepositEntry, RpcExecUpdate, RpcSyncStatus,
};
use strata_sequencer::{
    block_template::{BlockCompletionData, BlockGenerationConfig, BlockTemplate},
    duty::types::Duty,
};
use strata_state::{
    block::L2Block, client_state::ClientState, id::L2BlockId, operation::ClientUpdateOutput,
    sync_event::SyncEvent,
};
use zkaleido::ProofReceipt;

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "strata"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "strata"))]
pub trait StrataApi {
    /// Get blocks at a certain height
    #[method(name = "getBlocksAtIdx")]
    async fn get_blocks_at_idx(&self, idx: u64) -> RpcResult<Vec<HexBytes32>>;

    #[method(name = "protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<u64>;

    #[method(name = "blockTime")]
    async fn block_time(&self) -> RpcResult<u64>;

    #[method(name = "l1connected")]
    async fn get_l1_connection_status(&self) -> RpcResult<bool>;

    #[method(name = "l1status")]
    async fn get_l1_status(&self) -> RpcResult<RpcL1Status>;

    #[method(name = "getL1blockHash")]
    async fn get_l1_block_hash(&self, height: u64) -> RpcResult<Option<String>>;

    #[method(name = "clientStatus")]
    async fn get_client_status(&self) -> RpcResult<RpcClientStatus>;

    #[method(name = "getRecentBlockHeaders")]
    async fn get_recent_block_headers(&self, count: u64) -> RpcResult<Vec<RpcBlockHeader>>;

    #[method(name = "getHeadersAtIdx")]
    async fn get_headers_at_idx(&self, index: u64) -> RpcResult<Option<Vec<RpcBlockHeader>>>;

    #[method(name = "getHeaderById")]
    async fn get_header_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<RpcBlockHeader>>;

    #[method(name = "getExecUpdateById")]
    async fn get_exec_update_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<RpcExecUpdate>>;

    /// Fetches the list of epoch commitments that have been stored for an
    /// epoch.
    #[method(name = "getEpochCommitments")]
    async fn get_epoch_commitments(&self, epoch: u64) -> RpcResult<Vec<EpochCommitment>>;

    /// Fetches a particular epoch summary.
    #[method(name = "getEpochSummary")]
    async fn get_epoch_summary(
        &self,
        epoch: u64,
        slot: u64,
        terminal: L2BlockId,
    ) -> RpcResult<Option<EpochSummary>>;

    #[method(name = "getChainstateRaw")]
    async fn get_chainstate_raw(&self, slot: u64) -> RpcResult<Vec<u8>>;

    #[method(name = "getCLBlockWitness")]
    async fn get_cl_block_witness_raw(&self, block_id: L2BlockId) -> RpcResult<Vec<u8>>;

    #[method(name = "getCurrentDeposits")]
    async fn get_current_deposits(&self) -> RpcResult<Vec<u32>>;

    #[method(name = "getCurrentDepositById")]
    async fn get_current_deposit_by_id(&self, deposit_id: u32) -> RpcResult<RpcDepositEntry>;

    // block sync methods
    #[method(name = "syncStatus")]
    async fn sync_status(&self) -> RpcResult<RpcSyncStatus>;

    /// Get blocks in range as raw bytes of borsh serialized `Vec<L2BlockBundle>`.
    /// `start_height` and `end_height` are inclusive.
    #[method(name = "getRawBundles")]
    async fn get_raw_bundles(&self, start_height: u64, end_height: u64) -> RpcResult<HexBytes>;

    #[method(name = "getRawBundleById")]
    async fn get_raw_bundle_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<HexBytes>>;

    /// Get message by scope, Currently either Deposit or Withdrawal
    #[method(name = "getBridgeMsgsByScope")]
    async fn get_msgs_by_scope(&self, scope: HexBytes) -> RpcResult<Vec<HexBytes>>;

    /// Submit raw messages
    #[method(name = "submitBridgeMsg")]
    async fn submit_bridge_msg(&self, raw_msg: HexBytes) -> RpcResult<()>;

    /// Get the operators' public key table that is used to sign transactions and messages.
    ///
    /// # Note
    ///
    /// The rollup chain state only has the [`XOnlyPublicKey`](bitcoin::secp256k1::XOnlyPublicKey).
    /// The [`PublicKey`](bitcoin::secp256k1::PublicKey) in the [`PublickeyTable`] is generated by
    /// assuming an even parity as per the final schema in [`BIP 340`](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki#design)
    // TODO: consider adding a separate method to get the message signing key which may not be the
    // same as the wallet key if we decide to change the signature scheme.
    #[method(name = "getActiveOperatorChainPubkeySet")]
    async fn get_active_operator_chain_pubkey_set(&self) -> RpcResult<PublickeyTable>;

    /// Get latest checkpoint info
    #[method(name = "getLatestCheckpointIndex")]
    async fn get_latest_checkpoint_index(&self, finalized: Option<bool>) -> RpcResult<Option<u64>>;

    /// Get nth checkpoint info if any
    #[method(name = "getCheckpointInfo")]
    async fn get_checkpoint_info(&self, idx: u64) -> RpcResult<Option<RpcCheckpointInfo>>;

    /// Get the checkpoint confirmation status if checkpoint exists
    #[method(name = "getCheckpointConfStatus")]
    async fn get_checkpoint_conf_status(
        &self,
        idx: u64,
    ) -> RpcResult<Option<RpcCheckpointConfStatus>>;

    /// Get the l2block status from its height
    /// This assumes that the block finalization is always sequential. i.e all the blocks before the
    /// last finalized block are also finalized
    #[method(name = "getL2BlockStatus")]
    async fn get_l2_block_status(&self, block_height: u64) -> RpcResult<L2BlockStatus>;

    /// Gets the sync event by index, if it exists.
    #[method(name = "getSyncEvent")]
    async fn get_sync_event(&self, idx: u64) -> RpcResult<Option<SyncEvent>>;

    /// Gets the index of the last written sync event.
    #[method(name = "getLastSyncEventIdx")]
    async fn get_last_sync_event_idx(&self) -> RpcResult<u64>;

    /// Gets the client update output produced as a result of the sync event idx given.
    #[method(name = "getClientUpdateOutput")]
    async fn get_client_update_output(&self, idx: u64) -> RpcResult<Option<ClientUpdateOutput>>;
}

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "strataadmin"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "strataadmin"))]
pub trait StrataAdminApi {
    /// Stop the node.
    #[method(name = "stop")]
    async fn stop(&self) -> RpcResult<()>;
}

/// rpc endpoints that are only available on sequencer
#[cfg_attr(not(feature = "client"), rpc(server))]
#[cfg_attr(feature = "client", rpc(server, client))]
pub trait StrataSequencerApi {
    /// Get the last broadcast entry
    #[method(name = "strata_getLastTxEntry")]
    async fn get_last_tx_entry(&self) -> RpcResult<Option<L1TxEntry>>;

    /// Get the broadcast entry by its idx
    #[method(name = "strata_getTxEntryByIdx")]
    async fn get_tx_entry_by_idx(&self, idx: u64) -> RpcResult<Option<L1TxEntry>>;

    /// Adds L1Write sequencer duty which will be executed by sequencer
    #[method(name = "strataadmin_submitDABlob")]
    async fn submit_da_blob(&self, blobdata: HexBytes) -> RpcResult<()>;

    /// Verifies and adds the submitted proof to the checkpoint database
    #[method(name = "strataadmin_submitCheckpointProof")]
    async fn submit_checkpoint_proof(&self, idx: u64, proof: ProofReceipt) -> RpcResult<()>;

    // TODO: rpc endpoints that deal with L1 writes are currently limited to sequencer
    // due to l1 writer using wallet rpcs. Move these to common rpc once writer
    // can be used independently

    /// Adds an equivalent entry to broadcaster database, which will eventually be broadcasted
    #[method(name = "strataadmin_broadcastRawTx")]
    async fn broadcast_raw_tx(&self, rawtx: HexBytes) -> RpcResult<Txid>;

    #[method(name = "strata_getTxStatus")]
    async fn get_tx_status(&self, txid: HexBytes32) -> RpcResult<Option<L1TxStatus>>;

    #[method(name = "strata_getSequencerDuties")]
    async fn get_sequencer_duties(&self) -> RpcResult<Vec<Duty>>;

    #[method(name = "strata_getBlockTemplate")]
    async fn get_block_template(&self, config: BlockGenerationConfig) -> RpcResult<BlockTemplate>;

    #[method(name = "strata_completeBlockTemplate")]
    async fn complete_block_template(
        &self,
        template_id: L2BlockId,
        completion: BlockCompletionData,
    ) -> RpcResult<L2BlockId>;

    #[method(name = "strata_completeCheckpointSignature")]
    async fn complete_checkpoint_signature(&self, idx: u64, sig: HexBytes64) -> RpcResult<()>;
}

/// rpc endpoints that are only available for debugging purpose and subject to change.
#[cfg_attr(not(feature = "client"), rpc(server))]
#[cfg_attr(feature = "client", rpc(server, client))]
pub trait StrataDebugApi {
    /// Get the block by its id
    #[method(name = "debug_getBlockById")]
    async fn get_block_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<L2Block>>;

    /// Get the ChainState at a certain index
    #[method(name = "debug_getChainstateAtIdx")]
    async fn get_chainstate_at_idx(&self, idx: u64) -> RpcResult<Option<RpcChainState>>;

    /// Get the ClientState at a certain index
    #[method(name = "debug_getClientStateAtIdx")]
    async fn get_clientstate_at_idx(&self, idx: u64) -> RpcResult<Option<ClientState>>;

    /// for exiting the client based on context
    #[method(name = "debug_bail")]
    async fn set_bail_context(&self, ctx: String) -> RpcResult<()>;

    /// Instructs a worker to pause or resume its working
    #[method(name = "debug_pause_resume")]
    async fn pause_resume_worker(&self, wtype: WorkerType, action: Action) -> RpcResult<bool>;
}
