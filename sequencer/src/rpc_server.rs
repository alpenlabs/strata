#![allow(unused)]

use std::sync::Arc;

use alpen_vertex_btcio::btcio_status::BtcioStatus;
use alpen_vertex_db::traits::Database;
use alpen_vertex_db::traits::L1DataProvider;
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
use tokio::sync::{oneshot, Mutex, RwLock};
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

pub struct AlpenRpcImpl<D: Database + Send + Sync + 'static>
where
    <D as alpen_vertex_db::traits::Database>::L1Prov: Send + Sync + 'static,
{
    l1_status: Arc<RwLock<BtcioStatus>>,
    database: Arc<D>,
    // TODO
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl<D: Database + Send + Sync + 'static> AlpenRpcImpl<D>
where
    <D as alpen_vertex_db::traits::Database>::L1Prov: Send + Sync + 'static,
{
    pub fn new(
        l1_status: Arc<RwLock<BtcioStatus>>,
        database: Arc<D>,
        stop_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            l1_status,
            database,
            stop_tx: Mutex::new(Some(stop_tx)),
        }
    }
}

#[async_trait]
impl<D: Database + Send + Sync + 'static> AlpenApiServer for AlpenRpcImpl<D>
where
    <D as alpen_vertex_db::traits::Database>::L1Prov: Send + Sync + 'static,
{
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
        let btcio_status = self.l1_status.read().await.clone();

        Ok(L1Status {
            bitcoin_rpc_connected: btcio_status.bitcoin_rpc_connected,
            cur_height: btcio_status.cur_height,
            cur_tip_blkid: btcio_status.cur_tip_blkid,
            last_update: btcio_status.last_update,
            last_rpc_error: btcio_status.last_rpc_error,
        })
    }

    async fn get_l1_connection_status(&self) -> RpcResult<bool> {
        Ok(self.l1_status.read().await.bitcoin_rpc_connected)
    }

    async fn get_l1_block_hash(&self, height: u64) -> RpcResult<String> {
        let block_manifest = self
            .database
            .l1_provider()
            .get_block_manifest(height)
            .unwrap()
            .unwrap();

        Ok(format!("{:?}", block_manifest.block_hash()))
    }
}
