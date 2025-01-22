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
use parking_lot::RwLock;
use strata_bridge_relay::relayer::RelayerHandle;
use strata_btcio::{broadcaster::L1BroadcastHandle, writer::EnvelopeHandle};
#[cfg(feature = "debug-utils")]
use strata_common::bail_manager::BAIL_SENDER;
use strata_consensus_logic::{
    checkpoint::CheckpointHandle, csm::state_tracker::reconstruct_state, sync_manager::SyncManager,
    util::verify_proof,
};
use strata_db::{
    traits::*,
    types::{CheckpointConfStatus, CheckpointProvingStatus, L1TxEntry, L1TxStatus},
};
use strata_primitives::{
    bridge::{OperatorIdx, PublickeyTable},
    buf::Buf32,
    hash,
    l1::payload::{L1Payload, PayloadDest, PayloadIntent},
    params::Params,
};
use strata_rpc_api::{
    StrataAdminApiServer, StrataApiServer, StrataDebugApiServer, StrataSequencerApiServer,
};
use strata_rpc_types::{
    errors::RpcServerError as Error, DaBlob, HexBytes, HexBytes32, HexBytes64, L2BlockStatus,
    RpcBlockHeader, RpcBridgeDuties, RpcChainState, RpcCheckpointConfStatus, RpcCheckpointInfo,
    RpcClientStatus, RpcDepositEntry, RpcExecUpdate, RpcL1Status, RpcSyncStatus,
};
use strata_rpc_utils::to_jsonrpsee_error;
use strata_sequencer::{
    block_template::{
        BlockCompletionData, BlockGenerationConfig, BlockTemplate, TemplateManagerHandle,
    },
    checkpoint::{verify_checkpoint_sig, CheckpointHandle},
    duty::types::{Duty, DutyEntry, DutyTracker},
};
use strata_state::{
    batch::{BatchCheckpoint, SignedBatchCheckpoint},
    block::{L2Block, L2BlockBundle},
    bridge_duties::BridgeDuty,
    bridge_ops::WithdrawalIntent,
    chain_state::Chainstate,
    client_state::ClientState,
    header::L2Header,
    id::L2BlockId,
    l1::L1BlockId,
    operation::ClientUpdateOutput,
    sync_event::SyncEvent,
};
use strata_status::StatusChannel;
use strata_storage::NodeStorage;
use tokio::sync::{oneshot, Mutex};
use tracing::*;
use zkaleido::ProofReceipt;

use crate::extractor::{extract_deposit_requests, extract_withdrawal_infos};

pub struct StrataRpcImpl {
    status_channel: StatusChannel,
    sync_manager: Arc<SyncManager>,
    storage: Arc<NodeStorage>,
    checkpoint_handle: Arc<CheckpointHandle>,
    relayer_handle: Arc<RelayerHandle>,
}

impl StrataRpcImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        status_channel: StatusChannel,
        sync_manager: Arc<SyncManager>,
        storage: Arc<NodeStorage>,
        checkpoint_handle: Arc<CheckpointHandle>,
        relayer_handle: Arc<RelayerHandle>,
    ) -> Self {
        Self {
            status_channel,
            sync_manager,
            storage,
            checkpoint_handle,
            relayer_handle,
        }
    }

    /// Gets a ref to the current client state as of the last update.
    async fn get_client_state(&self) -> ClientState {
        self.sync_manager.status_channel().client_state()
    }

    // TODO make these not return Arc

    /// Gets a clone of the current client state and fetches the chainstate that
    /// of the L2 block that it considers the tip state.
    async fn get_cur_states(&self) -> Result<(ClientState, Option<Arc<Chainstate>>), Error> {
        let cs = self.get_client_state().await;

        if cs.sync().is_none() {
            return Ok((cs, None));
        }

        let chs = self.status_channel.chain_state().map(Arc::new);

        Ok((cs, chs))
    }

    async fn get_last_checkpoint_chainstate(&self) -> Result<Option<Arc<Chainstate>>, Error> {
        let client_state = self.status_channel.client_state();

        let Some(last_checkpoint) = client_state.l1_view().last_finalized_checkpoint() else {
            return Ok(None);
        };

        // in current implementation, chainstate idx == l2 block idx
        let (_, end_commitment) = last_checkpoint.batch_info.l2_range;

        Ok(self
            .storage
            .chainstate()
            .get_toplevel_chainstate_async(end_commitment.slot())
            .await?
            .map(Arc::new))
    }

    async fn fetch_l2_block_ok(&self, blkid: &L2BlockId) -> Result<L2BlockBundle, Error> {
        self.fetch_l2_block(blkid)
            .await?
            .ok_or(Error::MissingL2Block(*blkid))
    }

    async fn fetch_l2_block(&self, blkid: &L2BlockId) -> Result<Option<L2BlockBundle>, Error> {
        self.storage
            .l2()
            .get_block_data_async(blkid)
            .map_err(Error::Db)
            .await
    }
}

fn conv_blk_header_to_rpc(blk_header: &impl L2Header) -> RpcBlockHeader {
    RpcBlockHeader {
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
impl StrataApiServer for StrataRpcImpl {
    async fn get_blocks_at_idx(&self, idx: u64) -> RpcResult<Vec<HexBytes32>> {
        let l2_blocks = self
            .storage
            .l2()
            .get_blocks_at_height_async(idx)
            .await
            .map_err(Error::Db)?;
        let block_ids = l2_blocks
            .iter()
            .map(HexBytes32::from)
            .collect::<Vec<HexBytes32>>();
        Ok(block_ids)
    }

    async fn protocol_version(&self) -> RpcResult<u64> {
        Ok(1)
    }

    async fn block_time(&self) -> RpcResult<u64> {
        Ok(self.sync_manager.params().rollup.block_time)
    }

    async fn get_l1_status(&self) -> RpcResult<RpcL1Status> {
        let l1s = self.status_channel.l1_status();
        Ok(RpcL1Status::from_l1_status(
            l1s,
            self.sync_manager.params().rollup().network,
        ))
    }

    async fn get_l1_connection_status(&self) -> RpcResult<bool> {
        Ok(self.get_l1_status().await?.bitcoin_rpc_connected)
    }

    async fn get_l1_block_hash(&self, height: u64) -> RpcResult<Option<String>> {
        let blk_manifest = self
            .storage
            .l1()
            .get_block_manifest_async(height)
            .map_err(Error::Db)
            .await?;
        Ok(blk_manifest.map(|mf| mf.block_hash().to_string()))
    }

    async fn get_client_status(&self) -> RpcResult<RpcClientStatus> {
        let sync_state = self.status_channel.sync_state();
        let l1_view = self.status_channel.l1_view();

        let last_l1 = l1_view.tip_blkid().cloned().unwrap_or_else(|| {
            // TODO figure out a better way to do this
            warn!("last L1 block not set in client state, returning zero");
            L1BlockId::from(Buf32::zero())
        });

        // Copy these out of the sync state, if they're there.
        let (chain_tip_blkid, finalized_blkid) = sync_state
            .map(|ss| (*ss.chain_tip_blkid(), *ss.finalized_blkid()))
            .unwrap_or_default();

        // FIXME make this load from cache, and put the data we actually want
        // here in the client state
        // FIXME error handling
        let slot: u64 = self
            .storage
            .l2()
            .get_block_data_async(&chain_tip_blkid)
            .map_err(Error::Db)
            .await?
            .map(|b| b.header().blockidx())
            .unwrap_or(u64::MAX);

        Ok(RpcClientStatus {
            chain_tip: *chain_tip_blkid.as_ref(),
            chain_tip_slot: slot,
            finalized_blkid: *finalized_blkid.as_ref(),
            last_l1_block: *last_l1.as_ref(),
            buried_l1_height: l1_view.buried_l1_height(),
        })
    }

    async fn get_recent_block_headers(&self, count: u64) -> RpcResult<Vec<RpcBlockHeader>> {
        // FIXME: sync state should have a block number
        let sync_state = self.status_channel.sync_state();
        let tip_blkid = *sync_state.ok_or(Error::ClientNotStarted)?.chain_tip_blkid();

        let fetch_limit = self.sync_manager.params().run().l2_blocks_fetch_limit;
        if count > fetch_limit {
            return Err(Error::FetchLimitReached(fetch_limit, count).into());
        }

        let mut output = Vec::new();
        let mut cur_blkid = tip_blkid;
        while output.len() < count as usize {
            let l2_blk = self.fetch_l2_block_ok(&cur_blkid).await?;
            output.push(conv_blk_header_to_rpc(l2_blk.header()));
            cur_blkid = *l2_blk.header().parent();
            if l2_blk.header().blockidx() == 0 || Buf32::from(cur_blkid).is_zero() {
                break;
            }
        }

        Ok(output)
    }

    async fn get_headers_at_idx(&self, idx: u64) -> RpcResult<Option<Vec<RpcBlockHeader>>> {
        let sync_state = self.status_channel.sync_state();
        let tip_blkid = *sync_state.ok_or(Error::ClientNotStarted)?.chain_tip_blkid();

        // check the tip idx
        let tip_block = self.fetch_l2_block_ok(&tip_blkid).await?;
        let tip_idx = tip_block.header().blockidx();

        if idx > tip_idx {
            return Ok(None);
        }

        let blocks = self
            .storage
            .l2()
            .get_blocks_at_height_async(idx)
            .map_err(Error::Db)
            .await?;

        if blocks.is_empty() {
            return Ok(None);
        }

        let mut headers = Vec::new();
        for blkid in blocks {
            let bundle = self.fetch_l2_block_ok(&blkid).await?;
            headers.push(conv_blk_header_to_rpc(bundle.header()));
        }

        Ok(Some(headers))
    }

    async fn get_header_by_id(&self, blkid: L2BlockId) -> RpcResult<Option<RpcBlockHeader>> {
        let block = self.fetch_l2_block(&blkid).await?;
        Ok(block.map(|block| conv_blk_header_to_rpc(block.header())))
    }

    async fn get_exec_update_by_id(&self, blkid: L2BlockId) -> RpcResult<Option<RpcExecUpdate>> {
        match self.fetch_l2_block(&blkid).await? {
            Some(block) => {
                let exec_update = block.exec_segment().update();

                let withdrawals = exec_update
                    .output()
                    .withdrawals()
                    .iter()
                    .map(|intent| {
                        WithdrawalIntent::new(*intent.amt(), intent.destination().clone())
                    })
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

                Ok(Some(RpcExecUpdate {
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

    async fn get_cl_block_witness_raw(&self, blkid: L2BlockId) -> RpcResult<Vec<u8>> {
        let l2_blk_bundle = self.fetch_l2_block_ok(&blkid).await?;

        let prev_slot = l2_blk_bundle.block().header().header().blockidx() - 1;

        let chain_state = self
            .storage
            .chainstate()
            .get_toplevel_chainstate_async(prev_slot)
            .map_err(Error::Db)
            .await?
            .ok_or(Error::MissingChainstate(prev_slot))?;

        let cl_block_witness = (chain_state, l2_blk_bundle.block());
        let raw_cl_block_witness = borsh::to_vec(&cl_block_witness)
            .map_err(|_| Error::Other("Failed to get raw cl block witness".to_string()))?;

        Ok(raw_cl_block_witness)
    }

    async fn get_current_deposits(&self) -> RpcResult<Vec<u32>> {
        let deps = self
            .status_channel
            .deposits_table()
            .ok_or(Error::BeforeGenesis)?;

        Ok(deps.get_all_deposits_idxs_iters_iter().collect())
    }

    async fn get_current_deposit_by_id(&self, deposit_id: u32) -> RpcResult<RpcDepositEntry> {
        let deps = self
            .status_channel
            .deposits_table()
            .ok_or(Error::BeforeGenesis)?;
        Ok(deps
            .get_deposit(deposit_id)
            .ok_or(Error::UnknownIdx(deposit_id))
            .map(RpcDepositEntry::from_deposit_entry)?)
    }

    async fn sync_status(&self) -> RpcResult<RpcSyncStatus> {
        let sync_state = self.status_channel.sync_state();
        Ok(sync_state
            .map(|sync| RpcSyncStatus {
                tip_height: sync.chain_tip_height(),
                tip_block_id: *sync.chain_tip_blkid(),
                finalized_block_id: *sync.finalized_blkid(),
            })
            .ok_or(Error::ClientNotStarted)?)
    }

    async fn get_raw_bundles(&self, start_height: u64, end_height: u64) -> RpcResult<HexBytes> {
        let block_ids = futures::future::join_all(
            (start_height..=end_height)
                .map(|height| self.storage.l2().get_blocks_at_height_async(height)),
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
                .map(|blkid| self.storage.l2().get_block_data_async(blkid)),
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
            .storage
            .l2()
            .get_block_data_async(&block_id)
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
    ) -> RpcResult<RpcBridgeDuties> {
        info!(%operator_idx, %start_index, "received request for bridge duties");

        // OPTIMIZE: the extraction of deposit and withdrawal duties can happen in parallel as they
        // depend on independent sources of information. This optimization can be done if this RPC
        // call takes a lot of time (for example, when there are hundreds of thousands of
        // deposits/withdrawals).

        let network = self.sync_manager.params().rollup().network;

        let (deposit_duties, latest_index) =
            extract_deposit_requests(self.storage.l1().as_ref(), start_index, network).await?;

        let deposit_duties = deposit_duties.map(BridgeDuty::from);

        // withdrawal duties should only be generated from finalized checkpoint states
        let withdrawal_duties = self
            .get_last_checkpoint_chainstate()
            .await?
            .map(|chainstate| {
                extract_withdrawal_infos(chainstate.deposits_table())
                    .map(BridgeDuty::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut duties = vec![];
        duties.extend(deposit_duties);
        duties.extend(withdrawal_duties.into_iter());

        info!(%operator_idx, %start_index, "dispatching duties");
        Ok(RpcBridgeDuties {
            duties,
            start_index,
            stop_index: latest_index,
        })
    }

    async fn get_active_operator_chain_pubkey_set(&self) -> RpcResult<PublickeyTable> {
        let operator_table = self
            .status_channel
            .operator_table()
            .ok_or(Error::BeforeGenesis)?;
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

        Ok(entry.map(Into::into))
    }

    async fn get_checkpoint_conf_status(
        &self,
        idx: u64,
    ) -> RpcResult<Option<RpcCheckpointConfStatus>> {
        self.checkpoint_handle
            .get_checkpoint(idx)
            .await
            .map(|opt| opt.map(Into::into))
            .map_err(|e| Error::Checkpoint(e.to_string()).into())
    }

    async fn get_latest_checkpoint_index(&self, finalized: Option<bool>) -> RpcResult<Option<u64>> {
        let finalized = finalized.unwrap_or(false);
        if finalized {
            // get last finalized checkpoint index from state
            let (client_state, _) = self.get_cur_states().await?;
            Ok(client_state
                .l1_view()
                .last_finalized_checkpoint()
                .map(|checkpoint| checkpoint.batch_info.epoch()))
        } else {
            // get latest checkpoint index from db
            let idx = self
                .checkpoint_handle
                .get_last_checkpoint_idx()
                .await
                .map_err(|e| Error::Other(e.to_string()))?;

            Ok(idx)
        }
    }

    async fn get_l2_block_status(&self, block_height: u64) -> RpcResult<L2BlockStatus> {
        let sync_state = self.status_channel.sync_state();
        let l1_view = self.status_channel.l1_view();
        if let Some(last_checkpoint) = l1_view.last_finalized_checkpoint() {
            if last_checkpoint.batch_info.includes_l2_block(block_height) {
                return Ok(L2BlockStatus::Finalized(last_checkpoint.height));
            }
        }
        if let Some(l1_height) = l1_view.get_verified_l1_height(block_height) {
            return Ok(L2BlockStatus::Verified(l1_height));
        }

        if let Some(sync_status) = sync_state {
            if block_height < sync_status.chain_tip_height() {
                return Ok(L2BlockStatus::Confirmed);
            }
        }

        Ok(L2BlockStatus::Unknown)
    }

    // FIXME: possibly create a separate rpc type corresponding to SyncEvent
    async fn get_sync_event(&self, idx: u64) -> RpcResult<Option<SyncEvent>> {
        let ev: Option<SyncEvent> = self
            .storage
            .sync_event()
            .get_sync_event_async(idx)
            .await
            .map_err(Error::Db)?;

        Ok(ev)
    }

    async fn get_last_sync_event_idx(&self) -> RpcResult<u64> {
        let last = self
            .storage
            .sync_event()
            .get_last_idx_async()
            .await
            .map_err(Error::Db)?;

        // FIXME returning MAX if we haven't produced one yet, should figure
        // something else out
        Ok(last.unwrap_or(u64::MAX))
    }

    // FIXME: possibly create a separate rpc type corresponding to ClientUpdateOutput
    async fn get_client_update_output(&self, idx: u64) -> RpcResult<Option<ClientUpdateOutput>> {
        Ok(self
            .storage
            .client_state()
            .get_update_async(idx)
            .map_err(Error::Db)
            .await?)
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
    envelope_handle: Arc<EnvelopeHandle>,
    broadcast_handle: Arc<L1BroadcastHandle>,
    checkpoint_handle: Arc<CheckpointHandle>,
    template_manager_handle: TemplateManagerHandle,
    params: Arc<Params>,
    duty_tracker: Arc<RwLock<DutyTracker>>,
}

impl SequencerServerImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope_handle: Arc<EnvelopeHandle>,
        broadcast_handle: Arc<L1BroadcastHandle>,
        params: Arc<Params>,
        checkpoint_handle: Arc<CheckpointHandle>,
        template_manager_handle: TemplateManagerHandle,
        duty_tracker: Arc<RwLock<DutyTracker>>,
    ) -> Self {
        Self {
            envelope_handle,
            broadcast_handle,
            params,
            checkpoint_handle,
            template_manager_handle,
            duty_tracker,
        }
    }
}

#[async_trait]
impl StrataSequencerApiServer for SequencerServerImpl {
    async fn get_last_tx_entry(&self) -> RpcResult<Option<L1TxEntry>> {
        let broadcast_handle: Arc<L1BroadcastHandle> = self.broadcast_handle.clone();
        let txentry = broadcast_handle.get_last_tx_entry().await;
        Ok(txentry.map_err(|e| Error::Other(e.to_string()))?)
    }

    async fn get_tx_entry_by_idx(&self, idx: u64) -> RpcResult<Option<L1TxEntry>> {
        let broadcast_handle = &self.broadcast_handle;
        let txentry = broadcast_handle.get_tx_entry_by_idx_async(idx).await;
        Ok(txentry.map_err(|e| Error::Other(e.to_string()))?)
    }

    async fn submit_da_blob(&self, blob: HexBytes) -> RpcResult<()> {
        let commitment = hash::raw(&blob.0);
        let payload = L1Payload::new_da(blob.0);
        let blobintent = PayloadIntent::new(PayloadDest::L1, commitment, payload);
        // NOTE: It would be nice to return reveal txid from the submit method. But creation of txs
        // is deferred to signer in the writer module
        if let Err(e) = self.envelope_handle.submit_intent_async(blobintent).await {
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

    async fn submit_checkpoint_proof(
        &self,
        idx: u64,
        proof_receipt: ProofReceipt,
    ) -> RpcResult<()> {
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

        let checkpoint = entry.clone().into_batch_checkpoint();
        verify_proof(&checkpoint, &proof_receipt, self.params.rollup())
            .map_err(|e| Error::InvalidProof(idx, e.to_string()))?;

        entry.checkpoint.update_proof(proof_receipt.proof().clone());
        entry.proving_status = CheckpointProvingStatus::ProofReady;

        debug!(%idx, "Proof is pending, setting proof ready");

        self.checkpoint_handle
            .put_checkpoint(idx, entry)
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

    async fn get_sequencer_duties(&self) -> RpcResult<Vec<Duty>> {
        let duties = self
            .duty_tracker
            .read()
            .duties()
            .iter()
            .map(DutyEntry::duty)
            .cloned()
            .collect();
        Ok(duties)
    }

    async fn get_block_template(&self, config: BlockGenerationConfig) -> RpcResult<BlockTemplate> {
        self.template_manager_handle
            .generate_block_template(config)
            .await
            .map_err(to_jsonrpsee_error(""))
    }

    async fn complete_block_template(
        &self,
        template_id: L2BlockId,
        completion: BlockCompletionData,
    ) -> RpcResult<L2BlockId> {
        self.template_manager_handle
            .complete_block_template(template_id, completion)
            .await
            .map_err(to_jsonrpsee_error("failed to complete block template"))
    }

    async fn complete_checkpoint_signature(&self, idx: u64, sig: HexBytes64) -> RpcResult<()> {
        println!("complete_checkpoint_signature: {}; {:?}", idx, sig);
        let entry = self
            .checkpoint_handle
            .get_checkpoint(idx)
            .await
            .map_err(|e| Error::Other(e.to_string()))?
            .ok_or(Error::MissingCheckpointInDb(idx))?;

        if entry.proving_status != CheckpointProvingStatus::ProofReady {
            Err(Error::MissingCheckpointProof(idx))?;
        }

        if entry.confirmation_status == CheckpointConfStatus::Confirmed
            || entry.confirmation_status == CheckpointConfStatus::Finalized
        {
            Err(Error::CheckpointAlreadyPosted(idx))?;
        }

        let checkpoint = BatchCheckpoint::from(entry);
        let signed_checkpoint = SignedBatchCheckpoint::new(checkpoint, sig.0.into());

        if !verify_checkpoint_sig(&signed_checkpoint, &self.params) {
            Err(Error::InvalidCheckpointSignature(idx))?;
        }

        let payload = L1Payload::new_checkpoint(
            borsh::to_vec(&signed_checkpoint).map_err(|e| Error::Other(e.to_string()))?,
        );
        let sighash = signed_checkpoint.checkpoint().hash();

        let blob_intent = PayloadIntent::new(PayloadDest::L1, sighash, payload);
        self.envelope_handle
            .submit_intent_async(blob_intent)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        Ok(())
    }
}

pub struct StrataDebugRpcImpl {
    storage: Arc<NodeStorage>,
}

impl StrataDebugRpcImpl {
    pub fn new(storage: Arc<NodeStorage>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl StrataDebugApiServer for StrataDebugRpcImpl {
    async fn get_block_by_id(&self, block_id: L2BlockId) -> RpcResult<Option<L2Block>> {
        let l2_block = self
            .storage
            .l2()
            .get_block_data_async(&block_id)
            .await
            .map_err(Error::Db)?
            .map(|b| b.block().clone());
        Ok(l2_block)
    }

    async fn get_chainstate_at_idx(&self, idx: u64) -> RpcResult<Option<RpcChainState>> {
        let chain_state = self
            .storage
            .chainstate()
            .get_toplevel_chainstate_async(idx)
            .map_err(Error::Db)
            .await?;
        match chain_state {
            Some(cs) => Ok(Some(RpcChainState {
                tip_blkid: *cs.chain_tip_blkid(),
                tip_slot: cs.chain_tip_slot(),
                cur_epoch: cs.cur_epoch(),
            })),
            None => Ok(None),
        }
    }

    async fn get_clientstate_at_idx(&self, idx: u64) -> RpcResult<Option<ClientState>> {
        Ok(self
            .storage
            .client_state()
            .get_state_async(idx)
            .map_err(Error::Db)
            .await?)
    }

    async fn set_bail_context(&self, _ctx: String) -> RpcResult<()> {
        #[cfg(feature = "debug-utils")]
        let _sender = BAIL_SENDER.send(Some(_ctx));
        Ok(())
    }
}
