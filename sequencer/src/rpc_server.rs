#![allow(unused)]

use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorObject, ErrorObjectOwned},
};
use reth_primitives::{
    serde_helper::JsonStorageKey, Address, BlockId, BlockNumberOrTag, Bytes, B256, B64, U256, U64,
};
use reth_rpc_api::EthApiServer;
use reth_rpc_types::{
    serde_helpers::U64HexOrNumber, state::StateOverride, AccessListWithGasUsed,
    AnyTransactionReceipt, BlockOverrides, Bundle, EIP1186AccountProofResponse, EthCallResponse,
    FeeHistory, Header, Index, RichBlock, StateContext, SyncInfo, SyncStatus, Transaction,
    TransactionRequest, Work,
};
use thiserror::Error;
use tracing::*;

use alpen_vertex_rpc_api::AlpenApiServer;
use tokio::sync::{oneshot, Mutex};

#[derive(Debug, Error)]
pub enum Error {
    /// Unsupported RPCs for Vertex.  Some of these might need to be replaced
    /// with standard unsupported errors.
    #[error("unsupported RPC")]
    Unsupported,

    #[error("not yet implemented")]
    Unimplemented,

    /// Generic internal error message.  If this is used often it should be made
    /// into its own error type.
    #[error("{0}")]
    Other(String),

    /// Generic internal error message with a payload value.  If this is used
    /// often it should be made into its own error type.
    #[error("{0} (+data)")]
    OtherEx(String, serde_json::Value),
}

impl Error {
    pub fn code(&self) -> i32 {
        match self {
            Self::Unsupported => 1001,
            Self::Unimplemented => 1002,
            Self::Other(_) => 1100,
            Self::OtherEx(_, _) => 1101,
        }
    }
}

impl Into<ErrorObjectOwned> for Error {
    fn into(self) -> ErrorObjectOwned {
        let code = self.code();
        match self {
            Self::OtherEx(m, b) => ErrorObjectOwned::owned::<_>(code, format!("{m}"), Some(b)),
            _ => ErrorObjectOwned::owned::<serde_json::Value>(code, format!("{}", self), None),
        }
    }
}

pub struct AlpenRpcImpl {
    // TODO
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl AlpenRpcImpl {
    pub fn new(stop_tx: oneshot::Sender<()>) -> Self {
        Self {
            stop_tx: Mutex::new(Some(stop_tx)),
        }
    }
}

#[async_trait]
impl AlpenApiServer for AlpenRpcImpl {
    async fn protocol_version(&self) -> RpcResult<u64> {
        Ok(1)
    }

    async fn stop(&self) -> RpcResult<()> {
        let mut opt = self.stop_tx.lock().await;
        if let Some(stop_tx) = opt.take() {
            if stop_tx.send(()).is_err() {
                warn!("tried to send stop signal, channel closed");
            }
        }
        Ok(())
    }
}

/// Impl for the eth_ JSON-RPC interface.
///
/// See: https://github.com/paradigmxyz/reth/blob/main/crates/rpc/rpc-api/src/eth.rs
pub struct EthRpcImpl {
    // TODO
}

impl EthRpcImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl EthApiServer for EthRpcImpl {
    async fn protocol_version(&self) -> RpcResult<U64> {
        Err(Error::Unimplemented.into())
    }

    fn syncing(&self) -> RpcResult<SyncStatus> {
        Ok(SyncStatus::None)
    }

    async fn author(&self) -> RpcResult<Address> {
        Err(Error::Unsupported.into())
    }

    fn accounts(&self) -> RpcResult<Vec<Address>> {
        Err(Error::Unsupported.into())
    }

    fn block_number(&self) -> RpcResult<U256> {
        Err(Error::Unimplemented.into())
    }

    async fn chain_id(&self) -> RpcResult<Option<U64>> {
        // TODO change this
        Ok(Some(U64::from(2016)))
    }

    async fn block_by_hash(&self, hash: B256, full: bool) -> RpcResult<Option<RichBlock>> {
        Err(Error::Unimplemented.into())
    }

    async fn block_by_number(
        &self,
        number: BlockNumberOrTag,
        full: bool,
    ) -> RpcResult<Option<RichBlock>> {
        Err(Error::Unimplemented.into())
    }

    async fn block_transaction_count_by_hash(&self, hash: B256) -> RpcResult<Option<U256>> {
        Err(Error::Unimplemented.into())
    }

    async fn block_transaction_count_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> RpcResult<Option<U256>> {
        Err(Error::Unimplemented.into())
    }

    async fn block_uncles_count_by_hash(&self, hash: B256) -> RpcResult<Option<U256>> {
        Err(Error::Unimplemented.into())
    }

    async fn block_uncles_count_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> RpcResult<Option<U256>> {
        Err(Error::Unimplemented.into())
    }

    async fn block_receipts(
        &self,
        block_id: BlockId,
    ) -> RpcResult<Option<Vec<AnyTransactionReceipt>>> {
        Err(Error::Unimplemented.into())
    }

    async fn uncle_by_block_hash_and_index(
        &self,
        hash: B256,
        index: Index,
    ) -> RpcResult<Option<RichBlock>> {
        Err(Error::Unimplemented.into())
    }

    async fn uncle_by_block_number_and_index(
        &self,
        number: BlockNumberOrTag,
        index: Index,
    ) -> RpcResult<Option<RichBlock>> {
        Err(Error::Unimplemented.into())
    }

    async fn raw_transaction_by_hash(&self, hash: B256) -> RpcResult<Option<Bytes>> {
        Err(Error::Unimplemented.into())
    }

    async fn transaction_by_hash(&self, hash: B256) -> RpcResult<Option<Transaction>> {
        Err(Error::Unimplemented.into())
    }

    async fn raw_transaction_by_block_hash_and_index(
        &self,
        hash: B256,
        index: Index,
    ) -> RpcResult<Option<Bytes>> {
        Err(Error::Unimplemented.into())
    }

    async fn transaction_by_block_hash_and_index(
        &self,
        hash: B256,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        Err(Error::Unimplemented.into())
    }

    async fn raw_transaction_by_block_number_and_index(
        &self,
        number: BlockNumberOrTag,
        index: Index,
    ) -> RpcResult<Option<Bytes>> {
        Err(Error::Unimplemented.into())
    }

    async fn transaction_by_block_number_and_index(
        &self,
        number: BlockNumberOrTag,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        Err(Error::Unimplemented.into())
    }

    async fn transaction_receipt(&self, hash: B256) -> RpcResult<Option<AnyTransactionReceipt>> {
        Err(Error::Unimplemented.into())
    }

    async fn balance(&self, address: Address, block_number: Option<BlockId>) -> RpcResult<U256> {
        Err(Error::Unsupported.into())
    }

    async fn storage_at(
        &self,
        address: Address,
        index: JsonStorageKey,
        block_number: Option<BlockId>,
    ) -> RpcResult<B256> {
        Err(Error::Unimplemented.into())
    }

    async fn transaction_count(
        &self,
        address: Address,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256> {
        Err(Error::Unimplemented.into())
    }

    async fn get_code(&self, address: Address, block_number: Option<BlockId>) -> RpcResult<Bytes> {
        Err(Error::Unimplemented.into())
    }

    async fn header_by_number(&self, hash: BlockNumberOrTag) -> RpcResult<Option<Header>> {
        Err(Error::Unimplemented.into())
    }

    async fn header_by_hash(&self, hash: B256) -> RpcResult<Option<Header>> {
        Err(Error::Unimplemented.into())
    }

    async fn call(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> RpcResult<Bytes> {
        Err(Error::Unimplemented.into())
    }

    async fn call_many(
        &self,
        bundle: Bundle,
        state_context: Option<StateContext>,
        state_override: Option<StateOverride>,
    ) -> RpcResult<Vec<EthCallResponse>> {
        Err(Error::Unimplemented.into())
    }

    async fn create_access_list(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
    ) -> RpcResult<AccessListWithGasUsed> {
        Err(Error::Unimplemented.into())
    }

    async fn estimate_gas(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_override: Option<StateOverride>,
    ) -> RpcResult<U256> {
        Err(Error::Unimplemented.into())
    }

    async fn gas_price(&self) -> RpcResult<U256> {
        Err(Error::Unimplemented.into())
    }

    async fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        Err(Error::Unimplemented.into())
    }

    async fn blob_base_fee(&self) -> RpcResult<U256> {
        Err(Error::Unimplemented.into())
    }

    async fn fee_history(
        &self,
        block_count: U64HexOrNumber,
        newest_block: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        Err(Error::Unimplemented.into())
    }

    async fn is_mining(&self) -> RpcResult<bool> {
        Ok(false)
    }

    async fn hashrate(&self) -> RpcResult<U256> {
        Ok(U256::from(0))
    }

    async fn get_work(&self) -> RpcResult<Work> {
        Err(Error::Unsupported.into())
    }

    async fn submit_hashrate(&self, hashrate: U256, id: B256) -> RpcResult<bool> {
        Err(Error::Unsupported.into())
    }

    async fn submit_work(&self, nonce: B64, pow_hash: B256, mix_digest: B256) -> RpcResult<bool> {
        Err(Error::Unsupported.into())
    }

    async fn send_transaction(&self, request: TransactionRequest) -> RpcResult<B256> {
        Err(Error::Unsupported.into())
    }

    async fn send_raw_transaction(&self, bytes: Bytes) -> RpcResult<B256> {
        Err(Error::Unimplemented.into())
    }

    async fn sign(&self, address: Address, message: Bytes) -> RpcResult<Bytes> {
        Err(Error::Unsupported.into())
    }

    async fn sign_transaction(&self, transaction: TransactionRequest) -> RpcResult<Bytes> {
        Err(Error::Unsupported.into())
    }

    async fn sign_typed_data(&self, address: Address, data: serde_json::Value) -> RpcResult<Bytes> {
        Err(Error::Unsupported.into())
    }

    async fn get_proof(
        &self,
        address: Address,
        keys: Vec<JsonStorageKey>,
        block_number: Option<BlockId>,
    ) -> RpcResult<EIP1186AccountProofResponse> {
        Err(Error::Unimplemented.into())
    }
}
