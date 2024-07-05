#![allow(unused)]

use std::ops::Deref;

use alpen_vertex_state::client_state::ClientState;
use async_trait::async_trait;
use jsonrpsee::{
    core::{client, RpcResult},
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

use alpen_vertex_rpc_api::{AlpenApiServer, L1Status, L2Status};

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
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
    l2_status_rx: watch::Receiver<Option<ClientState>>
}

impl AlpenRpcImpl {
    pub fn new(stop_tx: oneshot::Sender<()>, l2_status_rx: watch::Receiver<Option<ClientState>>) -> Self {
        Self {
            stop_tx: Mutex::new(Some(stop_tx)),
            l2_status_rx
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

    async fn get_client_status(&self) -> RpcResult<L2Status> {
        warn!("alp_clientStatus not yet implemented");
        // borrow the current ClientState 
        let client_state = self.l2_status_rx.borrow().clone().unwrap();

        Ok(L2Status { 
            latest_l1_block:  client_state.recent_l1_blocks.last().unwrap().to_string(), 
            finalized_l2_tip: client_state.finalized_tip.to_string(), 
            buried_l1_height: client_state.buried_l1_height, 
            pending_deposits: client_state.chain_state.pending_deposits.len() as u64, 
            pending_withdrawals: client_state.chain_state.pending_withdraws.len() as u64, 
            accepted_l2_blocks: client_state.chain_state.accepted_l2_blocks.len() as u64
        })
    }
}
