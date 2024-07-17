#![allow(unused)]

use alpen_vertex_state::client_state::ClientState;
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorObject, ErrorObjectOwned},
};
use reth_primitives::{Address, BlockId, BlockNumberOrTag, Bytes, B256, B64, U256, U64};
use reth_rpc_api::EthApiServer;
use reth_rpc_types::{
    state::StateOverride, AccessListWithGasUsed, AnyTransactionReceipt, BlockOverrides, Bundle,
    EIP1186AccountProofResponse, EthCallResponse, FeeHistory, Header, Index, RichBlock,
    StateContext, SyncInfo, SyncStatus, Transaction, TransactionRequest, Work,
};
use thiserror::Error;
use tokio::sync::{oneshot, watch, Mutex};
use tracing::*;

use alpen_vertex_rpc_api::{AlpenApiServer, ClientStatus, L1Status};

#[derive(Debug, Error)]
pub enum Error {
    /// Unsupported RPCs for Vertex.  Some of these might need to be replaced
    /// with standard unsupported errors.
    #[error("unsupported RPC")]
    Unsupported,

    #[error("not yet implemented")]
    Unimplemented,
   
    #[error("client not started")]
    ClientNotStarted,

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
            Self::Unsupported => -32600,
            Self::Unimplemented => -32601,
            Self::Other(_) => -32000,
            Error::ClientNotStarted => -32001,
            Self::OtherEx(_, _) => -32000,
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
    client_state_rx: watch::Receiver<Option<ClientState>>,
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl AlpenRpcImpl {
    pub fn new(
        client_state_rx: watch::Receiver<Option<ClientState>>,
        stop_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            client_state_rx,
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

    async fn get_client_status(&self) -> RpcResult<ClientStatus> {
        if let Some(status) = self.client_state_rx.borrow().clone() {
            if let Some(last_l1_block) = status.recent_l1_block() {
                return Ok(ClientStatus {
                    chain_tip: *status.chain_tip_blkid().as_ref(),
                    finalized_blkid: *status.finalized_blkid().as_ref(),
                    last_l1_block: *last_l1_block.as_ref(),
                    buried_l1_height: status.buried_l1_height(),
                });
            }
        }

        return Err(Error::ClientNotStarted.into());
    }
}
