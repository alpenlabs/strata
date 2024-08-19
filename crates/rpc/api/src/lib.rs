//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.
use alpen_express_db::types::L1TxStatus;
use alpen_express_rpc_types::{
    types::{BlockHeader, ClientStatus, DepositEntry, ExecUpdate, L1Status},
    L2BlockId,
};
use alpen_express_state::{bridge_duties::BridgeDuties, bridge_ops::WithdrawalBatch};
use express_bridge_txm::{DepositInfo, ReimbursementRequest};

use bitcoin::secp256k1::schnorr::Signature;
use bitcoin::{OutPoint, Transaction, Txid};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alp"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alp"))]
pub trait AlpenApi {
    // TODO the rest of these
    #[method(name = "protocolVersion")]
    async fn protocol_version(&self) -> RpcResult<u64>;

    // TODO make this under the admin RPC interface
    #[method(name = "stop")]
    async fn stop(&self) -> RpcResult<()>;

    #[method(name = "l1connected")]
    async fn get_l1_connection_status(&self) -> RpcResult<bool>;

    #[method(name = "l1status")]
    async fn get_l1_status(&self) -> RpcResult<L1Status>;

    #[method(name = "getL1blockHash")]
    async fn get_l1_block_hash(&self, height: u64) -> RpcResult<Option<String>>;

    #[method(name = "clientStatus")]
    async fn get_client_status(&self) -> RpcResult<ClientStatus>;

    #[method(name = "getRecentBlocks")]
    async fn get_recent_blocks(&self, count: u64) -> RpcResult<Vec<BlockHeader>>;

    #[method(name = "getBlocksAtIdx")]
    async fn get_blocks_at_idx(&self, index: u64) -> RpcResult<Option<Vec<BlockHeader>>>;

    #[method(name = "getBlockById")]
    async fn get_block_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<BlockHeader>>;

    #[method(name = "getExecUpdateById")]
    async fn get_exec_update_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<ExecUpdate>>;

    #[method(name = "getCurrentDeposits")]
    async fn get_current_deposits(&self) -> RpcResult<Vec<u32>>;

    #[method(name = "getCurrentDepositById")]
    async fn get_current_deposit_by_id(&self, deposit_id: u32) -> RpcResult<DepositEntry>;

    #[method(name = "getTxStatus")]
    async fn get_tx_status(&self, txid: HexBytes32) -> RpcResult<Option<L1TxStatus>>;
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct HexBytes(#[serde_as(as = "serde_with::hex::Hex")] pub Vec<u8>);

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct HexBytes32(#[serde_as(as = "serde_with::hex::Hex")] pub [u8; 32]);

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpadmin"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpadmin"))]
pub trait AlpenAdminApi {
    #[method(name = "submitDABlob")]
    /// Adds L1Write sequencer duty which will be executed by sequencer
    async fn submit_da_blob(&self, blobdata: HexBytes) -> RpcResult<()>;

    #[method(name = "broadcastRawTx")]
    /// Adds an equivalent entry to broadcaster database, which will eventually be broadcasted
    async fn broadcast_raw_tx(&self, rawtx: HexBytes) -> RpcResult<Txid>;
}

/// APIs that are invoked by the bridge client to query and execute its duties.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridge"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridge"))]
// TODO: Add RPCs to handle the `BridgeMessage` as per [EXP-107](https://alpenlabs.atlassian.net/browse/EXP-107).
pub trait AlpenBridgeApi {
    /// Get relevant duties after a given block height in the rollup till the current block height.
    ///
    /// These duties could be extracted from the chainstate in the rollup or through the bridge p2p
    /// messaging queue.
    #[method(name = "getDuties")]
    async fn get_duties(&self, from_height: u64) -> RpcResult<BridgeDuties>;

    /// Broadcast the signature for a deposit request to other bridge clients.
    #[method(name = "broadcastDepositSignature")]
    async fn broadcast_deposit_signature(
        &self,
        deposit_info: DepositInfo,
        signature: Signature,
    ) -> RpcResult<()>;

    /// Broadcast request for signatures on withdrawal reimbursement from other bridge clients.
    #[method(name = "broadcastReimbursementRequest")]
    async fn broadcast_reimbursement_request(&self, request: ReimbursementRequest)
        -> RpcResult<()>;

    /// Get the details of the withdrawal assignee based on the UTXO.
    ///
    /// This is useful for validating whether a given withdrawal request was assigned to a given
    /// operator as an OutPoint is guaranteed to be unique.
    #[method(name = "getAssigneeDetails")]
    async fn get_assignee_details(
        &self,
        deposit_outpoint: OutPoint,
    ) -> RpcResult<Option<WithdrawalBatch>>;

    /// Broadcast fully signed transactions.
    #[method(name = "broadcastTx")]
    async fn broadcast_transactions(&self, txs: Vec<Transaction>) -> RpcResult<()>;
}
