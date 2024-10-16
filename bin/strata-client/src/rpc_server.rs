use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use bitcoin::{
    consensus::deserialize,
    hashes::Hash,
    key::Parity,
    secp256k1::{PublicKey, XOnlyPublicKey},
    Transaction as BTransaction, Txid,
};
use futures::TryFutureExt;
use jsonrpsee::core::RpcResult;
use strata_bridge_relay::relayer::RelayerHandle;
use strata_btcio::{broadcaster::L1BroadcastHandle, writer::InscriptionHandle};
use strata_consensus_logic::{
    checkpoint::CheckpointHandle, l1_handler::verify_proof, sync_manager::SyncManager,
};
use strata_db::{
    traits::*,
    types::{CheckpointProvingStatus, L1TxEntry, L1TxStatus},
};
use strata_primitives::{
    bridge::{OperatorIdx, PublickeyTable},
    buf::Buf32,
    hash,
    params::Params,
};
use strata_rpc_api::{StrataAdminApiServer, StrataApiServer, StrataSequencerApiServer};
use strata_rpc_types::{
    errors::RpcServerError as Error, BlockHeader, BridgeDuties, ClientStatus, DaBlob, ExecUpdate,
    HexBytes, HexBytes32, L1Status, L2BlockStatus, NodeSyncStatus, RpcCheckpointInfo,
};
use strata_rpc_utils::to_jsonrpsee_error;
use strata_state::{
    batch::BatchCheckpoint,
    block::L2BlockBundle,
    bridge_duties::BridgeDuty,
    bridge_ops::WithdrawalIntent,
    bridge_state::DepositEntry,
    chain_state::ChainState,
    client_state::ClientState,
    da_blob::{BlobDest, BlobIntent},
    header::L2Header,
    id::L2BlockId,
    l1::L1BlockId,
    operation::ClientUpdateOutput,
    sync_event::SyncEvent,
};
use strata_status::StatusRx;
use strata_storage::L2BlockManager;
use tokio::sync::{oneshot, Mutex};
use tracing::*;

use crate::extractor::{extract_deposit_requests, extract_withdrawal_infos};

fn fetch_l2blk<D: Database + Sync + Send + 'static>(
    l2_prov: &Arc<<D as Database>::L2DataProv>,
    blkid: L2BlockId,
) -> Result<L2BlockBundle, Error> {
    l2_prov
        .get_block_data(blkid)
        .map_err(Error::Db)?
        .ok_or(Error::MissingL2Block(blkid))
}

pub struct StrataRpcImpl<D> {
    status_rx: Arc<StatusRx>,
    database: Arc<D>,
    sync_manager: Arc<SyncManager>,
    l2_block_manager: Arc<L2BlockManager>,
    checkpoint_handle: Arc<CheckpointHandle>,
    relayer_handle: Arc<RelayerHandle>,
}

impl<D: Database + Sync + Send + 'static> StrataRpcImpl<D> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        status_rx: Arc<StatusRx>,
        database: Arc<D>,
        sync_manager: Arc<SyncManager>,
        l2_block_manager: Arc<L2BlockManager>,
        checkpoint_handle: Arc<CheckpointHandle>,
        relayer_handle: Arc<RelayerHandle>,
    ) -> Self {
        Self {
            status_rx,
            database,
            sync_manager,
            l2_block_manager,
            checkpoint_handle,
            relayer_handle,
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

            let chs_prov = db.chain_state_provider();
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
impl<D: Database + Send + Sync + 'static> StrataApiServer for StrataRpcImpl<D> {
    async fn protocol_version(&self) -> RpcResult<u64> {
        Ok(1)
    }

    async fn block_time(&self) -> RpcResult<u64> {
        Ok(self.sync_manager.params().rollup.block_time)
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

    async fn get_recent_block_headers(&self, count: u64) -> RpcResult<Vec<BlockHeader>> {
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

    async fn get_headers_at_idx(&self, idx: u64) -> RpcResult<Option<Vec<BlockHeader>>> {
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

    async fn get_header_by_id(&self, blkid: L2BlockId) -> RpcResult<Option<BlockHeader>> {
        let db = self.database.clone();
        // let blkid = L2BlockId::from(Buf32::from(blkid.0));

        Ok(wait_blocking("fetch_block", move || {
            let l2_prov = db.l2_provider();

            fetch_l2blk::<D>(l2_prov, blkid)
        })
        .await
        .map(|blk| conv_blk_header_to_rpc(blk.header()))
        .ok())
    }

    async fn get_exec_update_by_id(&self, blkid: L2BlockId) -> RpcResult<Option<ExecUpdate>> {
        let db = self.database.clone();
        // let blkid = L2BlockId::from(Buf32::from(blkid.0));

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

    async fn get_cl_block_witness_raw(&self, idx: u64) -> RpcResult<Option<Vec<u8>>> {
        let blk_manifest_db = self.database.clone();
        let blk_ids: Vec<L2BlockId> = wait_blocking("l2_blockid", move || {
            blk_manifest_db
                .clone()
                .l2_provider()
                .get_blocks_at_height(idx)
                .map_err(Error::Db)
        })
        .await?;

        // Check if blk_ids is empty
        let blkid = match blk_ids.first() {
            Some(id) => id.to_owned(),
            None => return Ok(None),
        };

        let l2_blk_db = self.database.clone();
        let l2_blk_bundle = wait_blocking("l2_block", move || {
            let l2_prov = l2_blk_db.l2_provider();
            fetch_l2blk::<D>(l2_prov, blkid).map_err(|_| Error::MissingL2Block(blkid))
        })
        .await?;

        let chain_state_db = self.database.clone();
        let chain_state = wait_blocking("l2_chain_state", move || {
            let cs_provider = chain_state_db.chain_state_provider();

            cs_provider
                .get_toplevel_state(idx - 1)
                .map_err(Error::Db)?
                .ok_or(Error::MissingChainstate(idx - 1))
        })
        .await?;

        let cl_block_witness = (chain_state, l2_blk_bundle.block());
        let raw_cl_block_witness = borsh::to_vec(&cl_block_witness)
            .map_err(|_| Error::Other("Failed to get raw cl block witness".to_string()))?;

        Ok(Some(raw_cl_block_witness))
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

        Ok(deposit_entry.clone())
    }

    async fn sync_status(&self) -> RpcResult<NodeSyncStatus> {
        let sync = {
            let cl = self.status_rx.cl.borrow();
            cl.sync().ok_or(Error::ClientNotStarted)?.clone()
        };
        Ok(NodeSyncStatus {
            tip_height: sync.chain_tip_height(),
            tip_block_id: *sync.chain_tip_blkid(),
            finalized_block_id: *sync.finalized_blkid(),
        })
    }

    async fn get_raw_bundles(&self, start_height: u64, end_height: u64) -> RpcResult<HexBytes> {
        let block_ids = futures::future::join_all(
            (start_height..=end_height)
                .map(|height| self.l2_block_manager.get_blocks_at_height_async(height)),
        )
        .await;

        let block_ids = block_ids
            .into_iter()
            .filter_map(|f| f.ok())
            .flatten()
            .collect::<Vec<_>>();

        let blocks = futures::future::join_all(
            block_ids
                .iter()
                .map(|blkid| self.l2_block_manager.get_block_async(blkid)),
        )
        .await;

        let blocks = blocks
            .into_iter()
            .filter_map(|blk| blk.ok().flatten())
            .collect::<Vec<_>>();

        borsh::to_vec(&blocks)
            .map(HexBytes)
            .map_err(to_jsonrpsee_error("failed to serialize"))
    }

    async fn get_raw_bundle_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<HexBytes>> {
        let block = self
            .l2_block_manager
            .get_block_async(&block_id)
            .await
            .map_err(|e| Error::Other(e.to_string()))?
            .map(|block| {
                borsh::to_vec(&block)
                    .map(HexBytes)
                    .map_err(to_jsonrpsee_error("failed to serialize"))
            })
            .transpose()?;
        Ok(block)
    }

    async fn get_msgs_by_scope(&self, scope: HexBytes) -> RpcResult<Vec<HexBytes>> {
        let msgs = self
            .relayer_handle
            .get_message_by_scope_async(scope.0)
            .map_err(to_jsonrpsee_error("querying relayer db"))
            .await?;

        let mut raw_msgs = Vec::new();
        for m in msgs {
            match borsh::to_vec(&m) {
                Ok(m) => raw_msgs.push(HexBytes(m)),
                Err(_) => {
                    let msg_id = m.compute_id();
                    warn!(%msg_id, "failed to serialize bridge msg");
                }
            }
        }

        Ok(raw_msgs)
    }

    async fn submit_bridge_msg(&self, raw_msg: HexBytes) -> RpcResult<()> {
        let msg =
            borsh::from_slice(&raw_msg.0).map_err(to_jsonrpsee_error("parse bridge message"))?;
        self.relayer_handle.submit_message_async(msg).await;
        Ok(())
    }

    // FIXME: find a way to handle reorgs if that becomes a problem
    async fn get_bridge_duties(
        &self,
        operator_idx: OperatorIdx,
        start_index: u64,
    ) -> RpcResult<BridgeDuties> {
        info!(%operator_idx, %start_index, "received request for bridge duties");

        // OPTIMIZE: the extraction of deposit and withdrawal duties can happen in parallel as they
        // depend on independent sources of information. This optimization can be done if this RPC
        // call takes a lot of time (for example, when there are hundreds of thousands of
        // deposits/withdrawals).

        let l1_db_provider = self.database.l1_provider();
        let network = self.status_rx.l1.borrow().network;

        let (deposit_duties, latest_index) =
            extract_deposit_requests(l1_db_provider, start_index, network).await?;

        let deposit_duties = deposit_duties.map(BridgeDuty::from);

        let (_, current_states) = self.get_cur_states().await?;
        let chain_state = current_states.ok_or(Error::BeforeGenesis)?;

        let withdrawal_duties = extract_withdrawal_infos(&chain_state).map(BridgeDuty::from);

        let mut duties = vec![];
        duties.extend(deposit_duties);
        duties.extend(withdrawal_duties);

        info!(%operator_idx, %start_index, "dispatching duties");
        Ok(BridgeDuties {
            duties,
            start_index,
            stop_index: latest_index,
        })
    }

    async fn get_active_operator_chain_pubkey_set(&self) -> RpcResult<PublickeyTable> {
        let (_, chain_state) = self.get_cur_states().await?;
        let chain_state = chain_state.ok_or(Error::BeforeGenesis)?;

        let operator_table = chain_state.operator_table();
        let operator_map: BTreeMap<OperatorIdx, PublicKey> = operator_table
            .operators()
            .iter()
            .fold(BTreeMap::new(), |mut map, entry| {
                let pubkey = XOnlyPublicKey::try_from(*entry.wallet_pk())
                    .expect("something has gone horribly wrong");

                // This is a taproot pubkey so its parity has to be even.
                let pubkey = pubkey.public_key(Parity::Even);

                map.insert(entry.idx(), pubkey);
                map
            });

        Ok(operator_map.into())
    }

    async fn get_checkpoint_info(&self, idx: u64) -> RpcResult<Option<RpcCheckpointInfo>> {
        let entry = self
            .checkpoint_handle
            .get_checkpoint(idx)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        let batch_comm: Option<BatchCheckpoint> = entry.map(Into::into);
        Ok(batch_comm.map(|bc| bc.batch_info().clone().into()))
    }

    async fn get_latest_checkpoint_index(&self) -> RpcResult<Option<u64>> {
        let idx = self
            .checkpoint_handle
            .get_last_checkpoint_idx()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        return Ok(idx);
    }

    async fn get_l2_block_status(&self, block_height: u64) -> RpcResult<L2BlockStatus> {
        let cl_state = self.get_client_state().await;
        if let Some(last_checkpoint) = cl_state.l1_view().last_finalized_checkpoint() {
            if last_checkpoint.batch_info.includes_l2_block(block_height) {
                return Ok(L2BlockStatus::Finalized(last_checkpoint.height));
            }
        }
        if let Some(l1_height) = cl_state.l1_view().get_verified_l1_height(block_height) {
            return Ok(L2BlockStatus::Verified(l1_height));
        }

        if let Some(sync_status) = cl_state.sync() {
            if block_height < sync_status.chain_tip_height() {
                return Ok(L2BlockStatus::Confirmed);
            }
        }

        Ok(L2BlockStatus::Unknown)
    }

    async fn get_sync_event(&self, idx: u64) -> RpcResult<Option<SyncEvent>> {
        let db = self.database.clone();

        let ev: Option<SyncEvent> = wait_blocking("fetch_sync_event", move || {
            Ok(db.sync_event_provider().get_sync_event(idx)?)
        })
        .await?;

        Ok(ev)
    }

    async fn get_last_sync_event_idx(&self) -> RpcResult<u64> {
        let db = self.database.clone();

        let last = wait_blocking("fetch_last_sync_event_idx", move || {
            Ok(db.sync_event_provider().get_last_idx()?)
        })
        .await?;

        // FIXME returning MAX if we haven't produced one yet, should figure
        // something else out
        Ok(last.unwrap_or(u64::MAX))
    }

    async fn get_client_update_output(&self, idx: u64) -> RpcResult<Option<ClientUpdateOutput>> {
        let db = self.database.clone();

        let res = wait_blocking("fetch_client_update_output", move || {
            let prov = db.client_state_provider();

            let w = prov.get_client_state_writes(idx)?;
            let a = prov.get_client_update_actions(idx)?;

            match (w, a) {
                (Some(w), Some(a)) => Ok(Some(ClientUpdateOutput::new(w, a))),
                // normally this is just that they're both missing
                _ => Ok(None),
            }
        })
        .await?;

        Ok(res)
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
    stop_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl AdminServerImpl {
    pub fn new(stop_tx: oneshot::Sender<()>) -> Self {
        Self {
            stop_tx: Mutex::new(Some(stop_tx)),
        }
    }
}

#[async_trait]
impl StrataAdminApiServer for AdminServerImpl {
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

pub struct SequencerServerImpl {
    inscription_handle: Arc<InscriptionHandle>,
    broadcast_handle: Arc<L1BroadcastHandle>,
    checkpoint_handle: Arc<CheckpointHandle>,
    params: Arc<Params>,
}

impl SequencerServerImpl {
    pub fn new(
        inscription_handle: Arc<InscriptionHandle>,
        broadcast_handle: Arc<L1BroadcastHandle>,
        params: Arc<Params>,
        checkpoint_handle: Arc<CheckpointHandle>,
    ) -> Self {
        Self {
            inscription_handle,
            broadcast_handle,
            params,
            checkpoint_handle,
        }
    }
}

#[async_trait]
impl StrataSequencerApiServer for SequencerServerImpl {
    async fn submit_da_blob(&self, blob: HexBytes) -> RpcResult<()> {
        let commitment = hash::raw(&blob.0);
        let blobintent = BlobIntent::new(BlobDest::L1, commitment, blob.0);
        // NOTE: It would be nice to return reveal txid from the submit method. But creation of txs
        // is deferred to signer in the writer module
        if let Err(e) = self
            .inscription_handle
            .submit_intent_async(blobintent)
            .await
        {
            return Err(Error::Other(e.to_string()).into());
        }
        Ok(())
    }

    async fn broadcast_raw_tx(&self, rawtx: HexBytes) -> RpcResult<Txid> {
        let tx: BTransaction = deserialize(&rawtx.0).map_err(|e| Error::Other(e.to_string()))?;
        let txid = tx.compute_txid();
        let dbid = *txid.as_raw_hash().as_byte_array();

        let entry = L1TxEntry::from_tx(&tx);

        self.broadcast_handle
            .put_tx_entry(dbid.into(), entry)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        Ok(txid)
    }

    async fn submit_checkpoint_proof(&self, idx: u64, proofbytes: HexBytes) -> RpcResult<()> {
        debug!(%idx, "received checkpoint proof request");
        let mut entry = self
            .checkpoint_handle
            .get_checkpoint(idx)
            .await
            .map_err(|e| Error::Other(e.to_string()))?
            .ok_or(Error::MissingCheckpointInDb(idx))?;
        debug!(%idx, "found checkpoint in db");

        // If proof is not pending error out
        if entry.proving_status != CheckpointProvingStatus::PendingProof {
            return Err(Error::ProofAlreadyCreated(idx))?;
        }

        entry.proof = strata_zkvm::Proof::new(proofbytes.into_inner());
        entry.proving_status = CheckpointProvingStatus::ProofReady;

        let checkpoint = entry.clone().into_batch_checkpoint();

        println!("*** Loing the Rollupprams");
        let ckp_batch_rollup_commitment = checkpoint.batch_info().rollup_params_commitment();
        println!("{:?}", ckp_batch_rollup_commitment);
        println!("***");
        verify_proof(&checkpoint, self.params.rollup())
            .map_err(|e| Error::InvalidProof(idx, e.to_string()))?;

        debug!(%idx, "Proof is pending, setting proof reaedy");

        self.checkpoint_handle
            .put_checkpoint_and_notify(idx, entry)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        debug!(%idx, "Success");

        Ok(())
    }

    async fn get_tx_status(&self, txid: HexBytes32) -> RpcResult<Option<L1TxStatus>> {
        let mut txid = txid.0;
        txid.reverse();
        let id = Buf32::from(txid);
        Ok(self
            .broadcast_handle
            .get_tx_status(id)
            .await
            .map_err(|e| Error::Other(e.to_string()))?)
    }
}
