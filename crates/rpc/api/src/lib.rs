//! Macro trait def for the `alp_` RPC namespace using jsonrpsee.
use alpen_express_db::types::L1TxStatus;
use alpen_express_primitives::bridge::OperatorIdx;
use alpen_express_rpc_types::{
    types::{BlockHeader, ClientStatus, DepositEntry, ExecUpdate, L1Status},
    L2BlockId,
};
use alpen_express_state::bridge_duties::BridgeDuties;
use bitcoin::{secp256k1::schnorr::Signature, Transaction, Txid};
use express_bridge_tx_builder::prelude::{CooperativeWithdrawalInfo, DepositInfo};
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
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HexBytes(#[serde_as(as = "serde_with::hex::Hex")] pub Vec<u8>);

impl AsRef<[u8]> for HexBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct HexBytes32(#[serde_as(as = "serde_with::hex::Hex")] pub [u8; 32]);

impl AsRef<[u8]> for HexBytes32 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

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

#[cfg_attr(not(feature = "client"), rpc(server, namespace = "alpbridgemsg"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "alpbridgemsg"))]
pub trait AlpenBridgeMsgApi {
    /// Get message by scope, Currently either Deposit or Withdrawal
    #[method(name = "getMsgsByScope")]
    async fn get_msgs_by_scope(&self, scope: HexBytes) -> RpcResult<Vec<HexBytes>>;

    /// Submit raw messages
    #[method(name = "submitRawMsg")]
    async fn submit_raw_msg(&self, raw_msg: HexBytes) -> RpcResult<()>;
}
