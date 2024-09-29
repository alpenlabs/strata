//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.
use alpen_express_db::types::L1TxStatus;
use alpen_express_primitives::bridge::{OperatorIdx, PublickeyTable};
use alpen_express_rpc_types::{
    types::{BlockHeader, ClientStatus, DepositEntry, ExecUpdate, L1Status},
    BridgeDuties, HexBytes, HexBytes32, NodeSyncStatus, RawBlockWitness, RpcCheckpointInfo,
};
use alpen_express_state::id::L2BlockId;
use bitcoin::Txid;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alp"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alp"))]
pub trait AlpenApi {
    #[method(name = "protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<u64>;

    #[method(name = "l1connected")]
    async fn get_l1_connection_status(&self) -> RpcResult<bool>;

    #[method(name = "l1status")]
    async fn get_l1_status(&self) -> RpcResult<L1Status>;

    #[method(name = "getL1blockHash")]
    async fn get_l1_block_hash(&self, height: u64) -> RpcResult<Option<String>>;

    #[method(name = "clientStatus")]
    async fn get_client_status(&self) -> RpcResult<ClientStatus>;

    #[method(name = "getRecentBlockHeaders")]
    async fn get_recent_block_headers(&self, count: u64) -> RpcResult<Vec<BlockHeader>>;

    #[method(name = "getHeadersAtIdx")]
    async fn get_headers_at_idx(&self, index: u64) -> RpcResult<Option<Vec<BlockHeader>>>;

    #[method(name = "getHeaderById")]
    async fn get_header_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<BlockHeader>>;

    #[method(name = "getExecUpdateById")]
    async fn get_exec_update_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<ExecUpdate>>;

    #[method(name = "getBlockWitness")]
    async fn get_block_witness_raw(&self, index: u64) -> RpcResult<Option<RawBlockWitness>>;

    #[method(name = "getCurrentDeposits")]
    async fn get_current_deposits(&self) -> RpcResult<Vec<u32>>;

    #[method(name = "getCurrentDepositById")]
    async fn get_current_deposit_by_id(&self, deposit_id: u32) -> RpcResult<DepositEntry>;

    // block sync methods
    #[method(name = "syncStatus")]
    async fn sync_status(&self) -> RpcResult<NodeSyncStatus>;

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

    /// Get the [`BridgeDuties`] from a certain `start_index` for a given [`OperatorIdx`].
    ///
    /// The `start_index` is a monotonically increasing number with no gaps. So, it is safe to call
    /// this method with any `u64` value. If an entry corresponding to the `start_index` is not
    /// found, an empty list is returned.
    #[method(name = "getBridgeDuties")]
    async fn get_bridge_duties(
        &self,
        operator_idx: OperatorIdx,
        start_index: u64,
    ) -> RpcResult<BridgeDuties>;

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

    /// Get nth checkpoint info if any
    #[method(name = "getCheckpointInfo")]
    async fn get_checkpoint_info(&self, idx: u64) -> RpcResult<Option<RpcCheckpointInfo>>;
}

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpadmin"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpadmin"))]
pub trait AlpenAdminApi {
    /// Stop the node.
    #[method(name = "stop")]
    async fn stop(&self) -> RpcResult<()>;
}

/// rpc endpoints that are only available on sequencer
#[cfg_attr(not(feature = "client"), rpc(server))]
#[cfg_attr(feature = "client", rpc(server, client))]
pub trait AlpenSequencerApi {
    /// Adds L1Write sequencer duty which will be executed by sequencer
    #[method(name = "alpadmin_submitDABlob")]
    async fn submit_da_blob(&self, blobdata: HexBytes) -> RpcResult<()>;

    /// Verifies and adds the submitted proof to the checkpoint database
    #[method(name = "alpadmin_submitCheckpointProof")]
    async fn submit_checkpoint_proof(&self, idx: u64, proof: HexBytes) -> RpcResult<()>;

    // TODO: rpc endpoints that deal with L1 writes are currently limited to sequencer
    // due to l1 writer using wallet rpcs. Move these to common rpc once writer
    // can be used independently

    /// Adds an equivalent entry to broadcaster database, which will eventually be broadcasted
    #[method(name = "alpadmin_broadcastRawTx")]
    async fn broadcast_raw_tx(&self, rawtx: HexBytes) -> RpcResult<Txid>;

    #[method(name = "alp_getTxStatus")]
    async fn get_tx_status(&self, txid: HexBytes32) -> RpcResult<Option<L1TxStatus>>;
}
