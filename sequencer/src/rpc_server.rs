#![allow(unused)]

use std::{borrow::BorrowMut, sync::Arc};

use alpen_express_btcio::{broadcaster::L1BroadcastHandle, writer::InscriptionHandle};
use alpen_express_consensus_logic::sync_manager::SyncManager;
use alpen_express_db::{
    traits::{ChainstateProvider, Database, L1DataProvider, L2DataProvider, SequencerDatabase},
    types::{L1TxEntry, L1TxStatus},
    DbResult,
};
use alpen_express_primitives::{buf::Buf32, hash};
use alpen_express_rpc_api::{AlpenAdminApiServer, AlpenApiServer, HexBytes, HexBytes32};
use alpen_express_rpc_types::{
    BlockHeader, ClientStatus, DaBlob, DepositEntry, DepositState, ExecUpdate, L1Status,
};
use alpen_express_state::{
    block::{L2Block, L2BlockBundle},
    bridge_ops::WithdrawalIntent,
    chain_state::ChainState,
    client_state::ClientState,
    da_blob::{BlobDest, BlobIntent},
    exec_update,
    header::{L2Header, SignedL2BlockHeader},
    id::L2BlockId,
    l1::L1BlockId,
};
use alpen_express_status::{StatusError, StatusRx};
use async_trait::async_trait;
use bitcoin::{consensus::deserialize, hashes::Hash, Transaction as BTransaction, Txid};
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
use tokio::sync::{
    mpsc::{self, Sender},
    oneshot, watch, Mutex, RwLock,
};
use tracing::*;

#[derive(Debug, Error)]
pub enum Error {
    /// Unsupported RPCs for express.  Some of these might need to be replaced
    /// with standard unsupported errors.
    #[error("unsupported RPC")]
    Unsupported,

    #[error("not yet implemented")]
    Unimplemented,

    #[error("client not started")]
    ClientNotStarted,

    #[error("missing L2 block {0:?}")]
    MissingL2Block(L2BlockId),

    #[error("missing L1 block manifest {0}")]
    MissingL1BlockManifest(u64),

    #[error("tried to call chain-related method before rollup genesis")]
    BeforeGenesis,

    #[error("unknown idx {0}")]
    UnknownIdx(u32),

    #[error("missing chainstate for index {0}")]
    MissingChainstate(u64),

    #[error("db: {0}")]
    Db(#[from] alpen_express_db::errors::DbError),

    #[error("blocking task '{0}' failed for unknown reason")]
    BlockingAbort(String),

    #[error("incorrect parameters: {0}")]
    IncorrectParameters(String),

    #[error("fetch limit reached. max {0}, provided {1}")]
    FetchLimitReached(u64, u64),

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
            Self::IncorrectParameters(_) => -32602,
            Self::MissingL2Block(_) => -32603,
            Self::MissingChainstate(_) => -32604,
            Self::Db(_) => -32605,
            Self::ClientNotStarted => -32606,
            Self::BeforeGenesis => -32607,
            Self::FetchLimitReached(_, _) => -32608,
            Self::UnknownIdx(_) => -32608,
            Self::MissingL1BlockManifest(_) => -32609,
            Self::BlockingAbort(_) => -32001,
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

fn fetch_l2blk<D: Database + Sync + Send + 'static>(
    l2_prov: &Arc<<D as Database>::L2Prov>,
    blkid: L2BlockId,
) -> Result<L2BlockBundle, Error> {
    l2_prov
        .get_block_data(blkid)
        .map_err(Error::Db)?
        .ok_or(Error::MissingL2Block(blkid))
}

pub struct AlpenRpcImpl<D> {
    status_rx: Arc<StatusRx>,
    database: Arc<D>,
    sync_manager: Arc<SyncManager>,
    bcast_handle: Arc<L1BroadcastHandle>,
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl<D: Database + Sync + Send + 'static> AlpenRpcImpl<D> {
    pub fn new(
        status_rx: Arc<StatusRx>,
        database: Arc<D>,
        sync_manager: Arc<SyncManager>,
        bcast_handle: Arc<L1BroadcastHandle>,
        stop_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            status_rx,
            database,
            sync_manager,
            bcast_handle,
            stop_tx: Mutex::new(Some(stop_tx)),
        }
    }

    /// Gets a ref to the current client state as of the last update.
    async fn get_client_state(&self) -> ClientState {
        self.sync_manager.status_rx().cl.borrow().clone()
    }

    /// Gets a clone of the current client state and fetches the chainstate that
    /// of the L2 block that it considers the tip state.
    async fn get_cur_states(&self) -> Result<(ClientState, Option<Arc<ChainState>>), Error> {
        let cs = self.get_client_state().await;

        if cs.sync().is_none() {
            return Ok((cs, None));
        }

        let ss = cs.sync().unwrap();
        let tip_blkid = *ss.chain_tip_blkid();

        let db = self.database.clone();
        let chs = wait_blocking("load_chainstate", move || {
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
        })
        .await?;

        Ok((cs, Some(chs)))
    }
}

fn conv_blk_header_to_rpc(blk_header: &impl L2Header) -> BlockHeader {
    BlockHeader {
        block_idx: blk_header.blockidx(),
        timestamp: blk_header.timestamp(),
        block_id: *blk_header.get_blockid().as_ref(),
        prev_block: *blk_header.parent().as_ref(),
        l1_segment_hash: *blk_header.l1_payload_hash().as_ref(),
        exec_segment_hash: *blk_header.exec_payload_hash().as_ref(),
        state_root: *blk_header.state_root().as_ref(),
    }
}

#[async_trait]
impl<D: Database + Send + Sync + 'static> AlpenApiServer for AlpenRpcImpl<D> {
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
        Ok(self.status_rx.l1.borrow().clone())
    }

    async fn get_l1_connection_status(&self) -> RpcResult<bool> {
        Ok(self.get_l1_status().await?.bitcoin_rpc_connected)
    }

    async fn get_l1_block_hash(&self, height: u64) -> RpcResult<Option<String>> {
        let db = self.database.clone();
        let blk_manifest = wait_blocking("l1_block_manifest", move || {
            db.l1_provider()
                .get_block_manifest(height)
                .map_err(|_| Error::MissingL1BlockManifest(height))
        })
        .await?;

        match blk_manifest {
            Some(blk) => Ok(Some(blk.block_hash().to_string())),
            None => Ok(None),
        }
    }

    async fn get_client_status(&self) -> RpcResult<ClientStatus> {
        let state = self.get_client_state().await;

        let last_l1 = state.most_recent_l1_block().copied().unwrap_or_else(|| {
            // TODO figure out a better way to do this
            warn!("last L1 block not set in client state, returning zero");
            L1BlockId::from(Buf32::zero())
        });

        // Copy these out of the sync state, if they're there.
        let (chain_tip, finalized_blkid) = state
            .sync()
            .map(|ss| (*ss.chain_tip_blkid(), *ss.finalized_blkid()))
            .unwrap_or_default();

        // FIXME make this load from cache, and put the data we actually want
        // here in the client state
        // FIXME error handling
        let db = self.database.clone();
        let slot: u64 = wait_blocking("load_cur_block", move || {
            let l2_prov = db.l2_provider();
            l2_prov
                .get_block_data(chain_tip)
                .map(|b| b.map(|b| b.header().blockidx()).unwrap_or(u64::MAX))
                .map_err(Error::from)
        })
        .await?;

        Ok(ClientStatus {
            chain_tip: *chain_tip.as_ref(),
            chain_tip_slot: slot,
            finalized_blkid: *finalized_blkid.as_ref(),
            last_l1_block: *last_l1.as_ref(),
            buried_l1_height: state.l1_view().buried_l1_height(),
        })
    }

    async fn get_recent_blocks(&self, count: u64) -> RpcResult<Vec<BlockHeader>> {
        // FIXME: sync state should have a block number
        let cl_state = self.get_client_state().await;

        let tip_blkid = *cl_state
            .sync()
            .ok_or(Error::ClientNotStarted)?
            .chain_tip_blkid();
        let db = self.database.clone();

        let fetch_limit = self.sync_manager.params().run().l2_blocks_fetch_limit;
        if count > fetch_limit {
            return Err(Error::FetchLimitReached(fetch_limit, count).into());
        }

        let blk_headers = wait_blocking("block_headers", move || {
            let l2_prov = db.l2_provider();
            let mut output = Vec::new();
            let mut cur_blkid = tip_blkid;

            while output.len() < count as usize {
                let l2_blk = fetch_l2blk::<D>(l2_prov, cur_blkid)?;
                output.push(conv_blk_header_to_rpc(l2_blk.header()));
                cur_blkid = *l2_blk.header().parent();
                if l2_blk.header().blockidx() == 0 || Buf32::from(cur_blkid).is_zero() {
                    break;
                }
            }

            Ok(output)
        })
        .await?;

        Ok(blk_headers)
    }

    async fn get_blocks_at_idx(&self, idx: u64) -> RpcResult<Option<Vec<BlockHeader>>> {
        let cl_state = self.get_client_state().await;
        let tip_blkid = *cl_state
            .sync()
            .ok_or(Error::ClientNotStarted)?
            .chain_tip_blkid();
        let db = self.database.clone();

        let blk_header = wait_blocking("block_at_idx", move || {
            let l2_prov = db.l2_provider();
            // check the tip idx
            let tip_idx = fetch_l2blk::<D>(l2_prov, tip_blkid)?.header().blockidx();

            if idx > tip_idx {
                return Ok(None);
            }

            l2_prov
                .get_blocks_at_height(idx)
                .map_err(Error::Db)?
                .iter()
                .map(|blkid| {
                    let l2_blk = fetch_l2blk::<D>(l2_prov, *blkid)?;

                    Ok(Some(conv_blk_header_to_rpc(l2_blk.block().header())))
                })
                .collect::<Result<Option<Vec<BlockHeader>>, Error>>()
        })
        .await?;

        Ok(blk_header)
    }

    async fn get_block_by_id(
        &self,
        blkid: alpen_express_rpc_types::L2BlockId,
    ) -> RpcResult<Option<BlockHeader>> {
        let db = self.database.clone();
        let blkid = L2BlockId::from(Buf32::from(blkid.0));

        Ok(wait_blocking("fetch_block", move || {
            let l2_prov = db.l2_provider();

            fetch_l2blk::<D>(l2_prov, blkid)
        })
        .await
        .map(|blk| conv_blk_header_to_rpc(blk.header()))
        .ok())
    }

    async fn get_exec_update_by_id(
        &self,
        blkid: alpen_express_rpc_types::L2BlockId,
    ) -> RpcResult<Option<ExecUpdate>> {
        let db = self.database.clone();
        let blkid = L2BlockId::from(Buf32::from(blkid.0));

        let l2_blk = wait_blocking("fetch_block", move || {
            let l2_prov = db.l2_provider();

            fetch_l2blk::<D>(l2_prov, blkid)
        })
        .await
        .ok();

        match l2_blk {
            Some(l2_blk) => {
                let exec_update = l2_blk.exec_segment().update();

                let withdrawals = exec_update
                    .output()
                    .withdrawals()
                    .iter()
                    .map(|intent| WithdrawalIntent::new(*intent.amt(), *intent.dest_pk()))
                    .collect();

                let da_blobs = exec_update
                    .output()
                    .da_blobs()
                    .iter()
                    .map(|blob| DaBlob {
                        dest: blob.dest().into(),
                        blob_commitment: *blob.commitment().as_ref(),
                    })
                    .collect();

                Ok(Some(ExecUpdate {
                    update_idx: exec_update.input().update_idx(),
                    entries_root: *exec_update.input().entries_root().as_ref(),
                    extra_payload: exec_update.input().extra_payload().to_vec(),
                    new_state: *exec_update.output().new_state().as_ref(),
                    withdrawals,
                    da_blobs,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_current_deposits(&self) -> RpcResult<Vec<u32>> {
        let (_, chain_state) = self.get_cur_states().await?;
        let chain_state = chain_state.ok_or(Error::BeforeGenesis)?;

        Ok(chain_state
            .deposits_table()
            .get_all_deposits_idxs_iters_iter()
            .collect())
    }

    async fn get_current_deposit_by_id(&self, deposit_id: u32) -> RpcResult<DepositEntry> {
        let (_, chain_state) = self.get_cur_states().await?;
        let chain_state = chain_state.ok_or(Error::BeforeGenesis)?;

        let deposit_entry = chain_state
            .deposits_table()
            .get_deposit(deposit_id)
            .ok_or(Error::UnknownIdx(deposit_id))?;

        let state = match deposit_entry.deposit_state() {
            alpen_express_state::bridge_state::DepositState::Created(_) => DepositState::Created,
            alpen_express_state::bridge_state::DepositState::Accepted => DepositState::Accepted,
            alpen_express_state::bridge_state::DepositState::Dispatched(_) => {
                DepositState::Dispatched
            }
            alpen_express_state::bridge_state::DepositState::Executed => DepositState::Executed,
        };

        Ok(DepositEntry {
            deposit_idx: deposit_id,
            amt: deposit_entry.amt(),
            state,
        })
    }

    async fn get_tx_status(&self, txid: HexBytes32) -> RpcResult<Option<L1TxStatus>> {
        let mut txid = txid.0;
        txid.reverse();
        let id = Buf32::from(txid);
        Ok(self
            .bcast_handle
            .get_tx_status(id)
            .await
            .map_err(|e| Error::Other(e.to_string()))?)
    }
}

/// Wrapper around [``tokio::task::spawn_blocking``] that handles errors in
/// external task and merges the errors into the standard RPC error type.
async fn wait_blocking<F, R>(name: &'static str, f: F) -> Result<R, Error>
where
    F: Fn() -> Result<R, Error> + Sync + Send + 'static,
    R: Sync + Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(v) => v,
        Err(_) => {
            error!(%name, "background task aborted for unknown reason");
            Err(Error::BlockingAbort(name.to_owned()))
        }
    }
}

pub struct AdminServerImpl {
    // Currently writer is Some() for sequencer only, but we need bcast_manager for both fullnode
    // and seq
    pub writer: Option<Arc<InscriptionHandle>>,
    pub bcast_handle: Arc<L1BroadcastHandle>,
}

impl AdminServerImpl {
    pub fn new(
        writer: Option<Arc<InscriptionHandle>>,
        bcast_handle: Arc<L1BroadcastHandle>,
    ) -> Self {
        Self {
            writer,
            bcast_handle,
        }
    }
}

#[async_trait]
impl AlpenAdminApiServer for AdminServerImpl {
    async fn submit_da_blob(&self, blob: HexBytes) -> RpcResult<()> {
        let commitment = hash::raw(&blob.0);
        let blobintent = BlobIntent::new(BlobDest::L1, commitment, blob.0);
        // NOTE: It would be nice to return reveal txid from the submit method. But creation of txs
        // is deferred to signer in the writer module
        if let Some(writer) = &self.writer {
            if let Err(e) = writer.submit_intent_async(blobintent).await {
                return Err(Error::Other("".to_string()).into());
            }
        }
        Ok(())
    }

    async fn broadcast_raw_tx(&self, rawtx: HexBytes) -> RpcResult<Txid> {
        let tx: BTransaction = deserialize(&rawtx.0).map_err(|e| Error::Other(e.to_string()))?;
        let txid = tx.compute_txid();
        let dbid = *txid.as_raw_hash().as_byte_array();

        let entry = L1TxEntry::from_tx(&tx);

        self.bcast_handle
            .insert_new_tx_entry(dbid.into(), entry)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        Ok(txid)
    }
}
