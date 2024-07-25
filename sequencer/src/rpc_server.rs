#![allow(unused)]

use std::sync::Arc;

use alpen_vertex_consensus_logic::sync_manager::SyncManager;
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
use tokio::sync::{oneshot, watch, Mutex, RwLock};

use alpen_vertex_db::traits::L1DataProvider;
use alpen_vertex_db::traits::{ChainstateProvider, Database, L2DataProvider};
use alpen_vertex_rpc_api::{AlpenApiServer, ClientStatus, L1Status};
use alpen_vertex_state::{
    chain_state::ChainState, client_state::ClientState, header::L2Header, id::L2BlockId,
};

use tracing::*;

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

    #[error("missing L2 block {0:?}")]
    MissingL2Block(L2BlockId),

    #[error("missing chainstate for index {0}")]
    MissingChainstate(u64),

    #[error("db: {0}")]
    Db(#[from] alpen_vertex_db::errors::DbError),

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
            Self::ClientNotStarted => -32602,
            Self::MissingL2Block(_) => -32603,
            Self::MissingChainstate(_) => -32604,
            Self::Db(_) => -32605,
            Self::Other(_) => -32000,
            Self::OtherEx(_, _) => -32000,
        }
    }
}

impl From<Error> for ErrorObjectOwned {
    fn from(val: Error) -> Self {
        let code = val.code();
        match val {
            Error::OtherEx(m, b) => ErrorObjectOwned::owned::<_>(code, m.to_string(), Some(b)),
            _ => ErrorObjectOwned::owned::<serde_json::Value>(code, format!("{}", val), None),
        }
    }
}

pub struct AlpenRpcImpl<D> {
    l1_status: Arc<RwLock<alpen_vertex_primitives::l1::L1Status>>,
    database: Arc<D>,
    sync_manager: Arc<SyncManager>,
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl<D: Database + Sync + Send + 'static> AlpenRpcImpl<D> {
    pub fn new(
        l1_status: Arc<RwLock<alpen_vertex_primitives::l1::L1Status>>,
        database: Arc<D>,
        sync_manager: Arc<SyncManager>,
        stop_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            l1_status,
            database,
            sync_manager,
            stop_tx: Mutex::new(Some(stop_tx)),
        }
    }

    /// Gets a ref to the current client state as of the last update.
    async fn get_client_state(&self) -> Arc<ClientState> {
        let cs_rx = self.sync_manager.create_state_watch_sub();
        let cs = cs_rx.borrow();
        cs.clone()
    }

    /// Gets a clone of the current client state and fetches the chainstate that
    /// of the L2 block that it considers the tip state.
    async fn get_cur_states(&self) -> Result<(Arc<ClientState>, Option<Arc<ChainState>>), Error> {
        let cs = self.get_client_state().await;

        if cs.sync().is_none() {
            return Ok((cs, None));
        }

        let ss = cs.sync().unwrap();
        let tip_blkid = *ss.chain_tip_blkid();

        let db = self.database.clone();
        let chs_fut = tokio::task::spawn_blocking::<_, Result<Arc<ChainState>, Error>>(move || {
            // FIXME this is horrible, the sync state should have the block
            // number in it somewhere
            let l2_prov = db.l2_provider();
            let tip_block = l2_prov
                .get_block_data(tip_blkid)?
                .ok_or(Error::MissingL2Block(tip_blkid))?;
            let idx = tip_block.header().blockidx();

            let chs_prov = db.chainstate_provider();
            let toplevel_st = chs_prov
                .get_toplevel_state(idx)?
                .ok_or(Error::MissingChainstate(idx))?;

            Ok(Arc::new(toplevel_st))
        });

        match chs_fut.await {
            Ok(res) => match res {
                Ok(v) => Ok((cs, Some(v))),
                Err(e) => Err(e.into()),
            },
            // This should only very rarely happen.
            _ => Err(Error::Other("internal server error".to_owned())),
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

    async fn get_client_status(&self) -> RpcResult<ClientStatus> {
        let state = self.get_client_state().await;

        let Some(last_l1) = state.recent_l1_block() else {
            warn!("last L1 block not set in client state, returning still not started");
            return Err(Error::ClientNotStarted.into());
        };

        // Copy these out of the sync state, if they're there.
        let (chain_tip, finalized_blkid) = state
            .sync()
            .map(|ss| (*ss.chain_tip_blkid(), *ss.finalized_blkid()))
            .unwrap_or_default();

        Ok(ClientStatus {
            chain_tip: *chain_tip.as_ref(),
            finalized_blkid: *finalized_blkid.as_ref(),
            last_l1_block: *last_l1.as_ref(),
            buried_l1_height: state.buried_l1_height(),
        })
    }
}
