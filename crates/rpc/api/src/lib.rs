//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.
use alpen_express_db::types::L1TxStatus;
use alpen_express_primitives::bridge::{OperatorIdx, PublickeyTable};
use alpen_express_rpc_types::{
    types::{BlockHeader, ClientStatus, DepositEntry, ExecUpdate, L1Status},
    HexBytes, HexBytes32, NodeSyncStatus, RawBlockWitness, RpcCheckpointInfo,
};
use alpen_express_state::{bridge_duties::BridgeDuties, id::L2BlockId};
use bitcoin::{secp256k1::schnorr::Signature, Transaction, Txid};
use express_bridge_tx_builder::prelude::{CooperativeWithdrawalInfo, DepositInfo};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alp"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alp"))]
pub trait AlpenApi {
    // TODO the rest of these
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

    #[method(name = "getTxStatus")]
    async fn get_tx_status(&self, txid: HexBytes32) -> RpcResult<Option<L1TxStatus>>;

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

    /// Get the bridge duties from a certain `block_height` for a given [`OperatorIdx`] along with
    /// the latest L1 `block_height` (for deposit duties).
    ///
    /// The `block_height` is a monotonically increasing number with no gaps. So, it is safe to call
    /// this method with any `u64` value. If an entry corresponding to the `block_height` is not
    /// found, an empty list is returned.
    #[method(name = "getBridgeDuties")]
    async fn get_bridge_duties(
        &self,
        operator_idx: OperatorIdx,
        block_height: u64,
    ) -> RpcResult<(BridgeDuties, u64)>;

    /// Get nth checkpoint info if any
    #[method(name = "getCheckpointInfo")]
    async fn get_checkpoint_info(&self, idx: u64) -> RpcResult<Option<RpcCheckpointInfo>>;

    /// Get the operators' public key table that is used to sign transactions.
    #[method(name = "getActiveOperatorChainPubkeySet")]
    async fn get_active_operator_chain_pubkey_set(&self) -> RpcResult<PublickeyTable>;
}

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpadmin"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpadmin"))]
pub trait AlpenAdminApi {
    /// Stop the node.
    #[method(name = "stop")]
    async fn stop(&self) -> RpcResult<()>;

    /// Adds L1Write sequencer duty which will be executed by sequencer
    #[method(name = "submitDABlob")]
    async fn submit_da_blob(&self, blobdata: HexBytes) -> RpcResult<()>;

    /// Adds an equivalent entry to broadcaster database, which will eventually be broadcasted
    #[method(name = "broadcastRawTx")]
    async fn broadcast_raw_tx(&self, rawtx: HexBytes) -> RpcResult<Txid>;

    /// Verifies and adds the submitted proof to the checkpoint database
    #[method(name = "submitCheckpointProof")]
    async fn submit_checkpoint_proof(
        &self,
        idx: u64,
        proof: HexBytes,
        state: HexBytes,
    ) -> RpcResult<()>;
}

/// APIs that are invoked by the bridge client to query and execute its duties.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
// TODO: Add RPCs to handle the `BridgeMessage` as per [EXP-107](https://alpenlabs.atlassian.net/browse/EXP-107).
pub trait AlpenBridgeApi {
    /// Get relevant duties after a certain checkpoint in the rollup in the current state for the
    /// operator with id `operator_idx`.
    ///
    /// These duties could be extracted from the chainstate in the rollup or through the bridge p2p
    /// messaging queue.
    #[method(name = "getDuties")]
    async fn get_duties(&self, operator_idx: OperatorIdx) -> RpcResult<BridgeDuties>;

    /// Broadcast the signature for a deposit request to other bridge clients.
    //  FIXME: this should actually send out a BridgeMessage after it has been implemented via [EXP-107](https://alpenlabs.atlassian.net/browse/EXP-107).
    #[method(name = "broadcastDepositSignature")]
    async fn broadcast_deposit_signature(
        &self,
        deposit_info: DepositInfo,
        signature: Signature,
    ) -> RpcResult<()>;

    /// Broadcast request for signatures on withdrawal reimbursement from other bridge clients.
    //  FIXME: this should actually send out a BridgeMessage after it has been implemented via [EXP-107](https://alpenlabs.atlassian.net/browse/EXP-107).
    #[method(name = "broadcastReimbursementRequest")]
    async fn broadcast_reimbursement_request(
        &self,
        request: CooperativeWithdrawalInfo,
    ) -> RpcResult<()>;

    /// Broadcast fully signed transactions.
    // TODO: this is a duplicate of an RPC in the `AlpenAdminApi`. Keeping it here so that the
    // bridge client only has to care about one RPC namespace i.e., `alpbridge`. But all of these
    // methods may move to another trait later.
    #[method(name = "broadcastTxs")]
    async fn broadcast_transactions(&self, txs: Vec<Transaction>) -> RpcResult<()>;
}
