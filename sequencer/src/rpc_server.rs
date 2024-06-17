#![allow(unused)]

use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorObject, ErrorObjectOwned},
};
use reth_primitives::{
    Address, BlockId, BlockNumberOrTag, Bytes, B256, B64, U256, U64,
};
use reth_rpc_api::EthApiServer;
use reth_rpc_types::{
    state::StateOverride, AccessListWithGasUsed,
    AnyTransactionReceipt, BlockOverrides, Bundle, EIP1186AccountProofResponse, EthCallResponse,
    FeeHistory, Header, Index, RichBlock, StateContext, SyncInfo, SyncStatus, Transaction,
    TransactionRequest, Work,
};
use thiserror::Error;
use tokio::sync::{oneshot, Mutex};
use tracing::*;

use alpen_vertex_rpc_api::{AlpenApiServer, L1Status};

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

    async fn get_l1_status(&self) -> RpcResult<L1Status> {
        // TODO implement this
        warn!("alp_l1status not yet implemented");
        Ok(L1Status {
            cur_height: 0,
            cur_tip_blkid: String::new(),
            last_update: 0,
        })
    }
}
