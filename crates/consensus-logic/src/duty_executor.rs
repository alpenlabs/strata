//! Executes duties.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use std::{thread, time};

use borsh::{BorshDeserialize, BorshSerialize};
use tokio::sync::broadcast;
use tracing::*;

use alpen_express_db::traits::{ClientStateProvider, Database, L2DataProvider, L2DataStore};
use alpen_express_evmctl::engine::{ExecEngineCtl, PayloadStatus};
use alpen_express_evmctl::errors::EngineError;
use alpen_express_evmctl::messages::{ExecPayloadData, PayloadEnv};
use alpen_express_primitives::buf::{Buf32, Buf64};
use alpen_express_primitives::params::RollupParams;
use alpen_express_state::block::{ExecSegment, L1Segment, L2BlockAccessory, L2BlockBundle};
use alpen_express_state::client_state::ClientState;
use alpen_express_state::exec_update::{ExecUpdate, UpdateOutput};
use alpen_express_state::header::L2BlockHeader;
use alpen_express_state::prelude::*;

use crate::credential::sign_schnorr_sig;
use crate::duties::{self, Duty, DutyBatch, Identity};
use crate::duty_extractor;
use crate::errors::Error;
use crate::message::{ClientUpdateNotif, ForkChoiceMessage};
use crate::sync_manager::SyncManager;

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub enum IdentityKey {
    Sequencer(Buf32),
}

/// Contains both the identity key used for signing and the identity used for
/// verifying signatures.  This is really just a stub that we should replace
/// with real cryptographic signatures and putting keys in the rollup params.
#[derive(Clone, Debug)]
pub struct IdentityData {
    pub ident: Identity,
    pub key: IdentityKey,
}

impl IdentityData {
    pub fn new(ident: Identity, key: IdentityKey) -> Self {
        Self { ident, key }
    }
}

pub fn duty_tracker_task<D: Database>(
    mut cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    batch_queue: broadcast::Sender<DutyBatch>,
    ident: Identity,
    database: Arc<D>,
) -> Result<(), Error> {
    let mut duties_tracker = duties::DutyTracker::new_empty();

    let idx = database.client_state_provider().get_last_checkpoint_idx()?;
    let last_checkpoint_state = database.client_state_provider().get_state_checkpoint(idx)?;
    let mut last_finalized_blk = match last_checkpoint_state {
        Some(state) => state.sync().map(|sync| *sync.finalized_blkid()),
        None => None,
    };

    loop {
        let update = match cupdate_rx.blocking_recv() {
            Ok(u) => u,
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                // TODO maybe check the things we missed, but this is fine for now
                warn!(%skipped, "overloaded, skipping indexing some duties");
                continue;
            }
        };

        let ev_idx = update.sync_event_idx();
        let new_state = update.new_state();
        trace!(%ev_idx, "new consensus state, updating duties");
        trace!("STATE: {new_state:#?}");

        if let Err(e) = update_tracker(
            &mut duties_tracker,
            new_state,
            &ident,
            database.as_ref(),
            &mut last_finalized_blk,
        ) {
            error!(err = %e, "failed to update duties tracker");
        }

        // Publish the new batch.
        let batch = DutyBatch::new(ev_idx, duties_tracker.duties().to_vec());
        if batch_queue.send(batch).is_err() {
            warn!("failed to publish new duties batch");
        }
    }

    info!("duty extractor task exiting");

    Ok(())
}

fn update_tracker<D: Database>(
    tracker: &mut duties::DutyTracker,
    state: &ClientState,
    ident: &Identity,
    database: &D,
    last_finalized_block: &mut Option<L2BlockId>,
) -> Result<(), Error> {
    let Some(ss) = state.sync() else {
        return Ok(());
    };

    let new_duties = duty_extractor::extract_duties(state, ident, database)?;

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let tip_blkid = *ss.chain_tip_blkid();
    let l2prov = database.l2_provider();
    let block = l2prov
        .get_block_data(tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?;
    let block_idx = block.header().blockidx();
    let ts = time::Instant::now(); // FIXME XXX use .timestamp()!!!

    // Figure out which blocks were finalized
    let new_finalized = state.sync().map(|sync| *sync.finalized_blkid());
    let newly_finalized_blocks: Vec<L2BlockId> =
        get_finalized_blocks(new_finalized, l2prov.as_ref(), last_finalized_block)?;

    let tracker_update = duties::StateUpdate::new(block_idx, ts, newly_finalized_blocks);
    let n_evicted = tracker.update(&tracker_update);
    trace!(%n_evicted, "evicted old duties from new consensus state");

    // Now actually insert the new duties.
    tracker.add_duties(tip_blkid, block_idx, new_duties.into_iter());

    Ok(())
}

fn get_finalized_blocks(
    finalized: Option<L2BlockId>,
    l2prov: &impl L2DataProvider,
    last_finalized_block: &mut Option<L2BlockId>,
) -> Result<Vec<L2BlockId>, Error> {
    // Figure out which blocks were finalized
    let mut newly_finalized_blocks: Vec<L2BlockId> = Vec::new();
    let mut new_finalized = finalized;

    loop {
        if let Some(finalized) = new_finalized {
            // If the last finalized block is equal to the new finalized block,
            // it means that no new blocks are finalized
            if *last_finalized_block == Some(finalized) {
                break;
            }

            // else loop till we reach to the last finalized block or go all the way
            // as long as we get some block data
            // TODO: verify if this works as expected
            newly_finalized_blocks.push(finalized);

            match l2prov.get_block_data(finalized)? {
                Some(block) => new_finalized = Some(*block.header().parent()),
                None => break,
            }
        }
    }

    // Update the last_finalized_block with the new_finalized value
    *last_finalized_block = finalized;

    Ok(newly_finalized_blocks)
}

pub fn duty_dispatch_task<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
>(
    mut updates: broadcast::Receiver<DutyBatch>,
    ident_key: IdentityKey,
    sync_man: Arc<SyncManager>,
    database: Arc<D>,
    engine: Arc<E>,
    pool: Arc<threadpool::ThreadPool>,
    params: &RollupParams,
) {
    // TODO make this actually work
    let mut pending_duties: HashMap<u64, ()> = HashMap::new();

    // TODO still need some stuff here to decide if we're fully synced and
    // *should* dispatch duties

    loop {
        let update = match updates.blocking_recv() {
            Ok(u) => u,
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!(%skipped, "overloaded, skipping dispatching some duties");
                continue;
            }
        };

        // TODO check pending_duties to remove any completed duties

        for duty in update.duties() {
            let id = duty.id();

            // Skip any duties we've already dispatched.
            if pending_duties.contains_key(&id) {
                continue;
            }

            // Clone some things, spawn the task, then remember the join handle.
            // TODO make this use a thread pool
            let d = duty.duty().clone();
            let ik = ident_key.clone();
            let sm = sync_man.clone();
            let db = database.clone();
            let e = engine.clone();
            let params = params.clone();
            pool.execute(move || duty_exec_task(d, ik, sm, db, e, params));
            trace!(%id, "dispatched duty exec task");
            pending_duties.insert(id, ());
        }
    }

    info!("duty dispatcher task exiting");
}

/// Toplevel function that's actually performs a job.  This is spawned on a/
/// thread pool so we don't have to worry about it blocking *too* much other
/// work.
fn duty_exec_task<D: Database, E: ExecEngineCtl>(
    duty: Duty,
    ik: IdentityKey,
    sync_man: Arc<SyncManager>,
    database: Arc<D>,
    engine: Arc<E>,
    params: RollupParams,
) {
    if let Err(e) = perform_duty(
        &duty,
        &ik,
        &sync_man,
        database.as_ref(),
        engine.as_ref(),
        params,
    ) {
        error!(err = %e, "error performing duty");
    } else {
        debug!("completed duty successfully");
    }
}

fn perform_duty<D: Database, E: ExecEngineCtl>(
    duty: &Duty,
    ik: &IdentityKey,
    sync_man: &SyncManager,
    database: &D,
    engine: &E,
    params: RollupParams,
) -> Result<(), Error> {
    match duty {
        Duty::SignBlock(data) => {
            let target = data.target_slot();
            let Some((blkid, _block)) = sign_and_store_block(target, ik, database, engine, params)?
            else {
                return Ok(());
            };

            // Submit it to the fork choice manager to update the consensus state
            // with it.
            let ctm = ForkChoiceMessage::NewBlock(blkid);
            if !sync_man.submit_chain_tip_msg(ctm) {
                error!(?blkid, "failed to submit new block to fork choice manager");
            }

            // TODO do we have to do something with _block right now?

            // TODO eventually, send the block out to peers

            Ok(())
        }
    }
}

fn sign_and_store_block<D: Database, E: ExecEngineCtl>(
    slot: u64,
    ik: &IdentityKey,
    database: &D,
    engine: &E,
    params: RollupParams,
) -> Result<Option<(L2BlockId, L2Block)>, Error> {
    debug!(%slot, "prepating to publish block");

    // Check the block we were supposed to build isn't already in the database,
    // if so then just republish that.  This checks that there just if we have a
    // block at that height, which for now is the same thing.
    let l2prov = database.l2_provider();
    let blocks_at_slot = l2prov.get_blocks_at_height(slot)?;
    if !blocks_at_slot.is_empty() {
        // FIXME Should we be more verbose about this?
        warn!(%slot, "was turn to propose block, but found block in database already");
        return Ok(None);
    }

    // TODO get the consensus state this duty was created in response to and
    // pull out the current tip block from it
    // XXX this is really bad as-is
    let cs_prov = database.client_state_provider();
    let ckpt_idx = cs_prov.get_last_checkpoint_idx()?; // FIXME: this isn't what this is for, it only works because we're checkpointing on every state
                                                       // right now
    let last_cstate = cs_prov
        .get_state_checkpoint(ckpt_idx)?
        .expect("dutyexec: get state checkpoint");

    let Some(last_ss) = last_cstate.sync() else {
        return Ok(None);
    };

    let prev_block_id = *last_ss.chain_tip_blkid();

    let prev_block = database
        .l2_provider()
        .get_block_data(prev_block_id)?
        .expect("prev block must exist");

    // TODO: get from rollup config
    let block_time = params.block_time;
    let target_ts = prev_block.block().header().timestamp() + block_time;
    let current_ts = now_millis();

    if current_ts < target_ts {
        sleep(Duration::from_millis(target_ts - current_ts));
    }

    // Start preparing the EL payload.
    let ts = now_millis();
    let safe_l1_block = Buf32::zero(); // TODO
    let payload_env = PayloadEnv::new(ts, prev_block_id, safe_l1_block, Vec::new());
    let key = engine.prepare_payload(payload_env)?;
    trace!(%slot, "submitted EL payload job, waiting for completion");

    // TODO Pull data from CSM state that we've observed from L1, including new
    // headers or any headers needed to perform a reorg if necessary.
    let l1_seg = L1Segment::new(Vec::new());

    // Wait 2 seconds for the block to be finished.
    // TODO Pull data from state about the new safe L1 hash, prev state roots,
    // etc. to assemble the payload env for this block.
    let wait = time::Duration::from_millis(100);
    let timeout = time::Duration::from_millis(3000);
    let Some(payload_data) = poll_status_loop(key, engine, wait, timeout)? else {
        // TODO better error message
        return Err(Error::Other("EL block assembly timed out".to_owned()));
    };
    trace!(%slot, "finished EL payload job");

    // TODO correctly assemble the exec segment, since this is bodging out how
    // the inputs/outputs should be structured
    let eui = payload_data.update_input().clone();
    let exec_update = ExecUpdate::new(eui, UpdateOutput::new_from_state(Buf32::zero()));
    let exec_seg = ExecSegment::new(exec_update);

    // Assemble the body and the header core.
    let body = L2BlockBody::new(l1_seg, exec_seg);
    let state_root = Buf32::zero(); // TODO compute this from the different parts
    let header = L2BlockHeader::new(slot, ts, prev_block_id, &body, state_root);
    let header_sig = sign_header(&header, ik);
    let signed_header = SignedL2BlockHeader::new(header, header_sig);
    let blkid = signed_header.get_blockid();
    let block_accessory = L2BlockAccessory::new(payload_data.accessory_data().to_vec());
    let final_block = L2Block::new(signed_header, body);
    let final_block_bundle = L2BlockBundle::new(final_block.clone(), block_accessory);
    info!(%slot, ?blkid, "finished building new block");

    // Store the block in the database.
    let l2store = database.l2_store();
    l2store.put_block_data(final_block_bundle)?;
    debug!(?blkid, "wrote block to datastore");

    Ok(Some((blkid, final_block)))
}

/// Signs the L2BlockHeader and returns the signature
fn sign_header(header: &L2BlockHeader, ik: &IdentityKey) -> Buf64 {
    let msg = header.get_sighash();
    match ik {
        IdentityKey::Sequencer(sk) => sign_schnorr_sig(&msg, sk),
    }
}

/// Returns the current unix time as milliseconds.
// TODO maybe we should use a time source that is possibly more consistent with
// the rest of the network for this?
fn now_millis() -> u64 {
    time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64
}

fn poll_status_loop<E: ExecEngineCtl>(
    job: u64,
    engine: &E,
    wait: time::Duration,
    timeout: time::Duration,
) -> Result<Option<ExecPayloadData>, EngineError> {
    let start = time::Instant::now();
    loop {
        // Sleep at the beginning since the first iter isn't likely to have it
        // ready.
        thread::sleep(wait);

        // Check the payload for the result.
        let payload = engine.get_payload_status(job)?;
        if let PayloadStatus::Ready(pl) = payload {
            return Ok(Some(pl));
        }

        // If we've waited too long now.
        if time::Instant::now() - start > timeout {
            warn!(%job, "payload build job timed out");
            break;
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::l2::gen_l2_chain;
    use alpen_vertex_db::traits::{Database, L2DataStore};
    use alpen_vertex_state::header::L2Header;

    use super::get_finalized_blocks;

    #[test]
    fn test_get_finalized_blocks() {
        let db = alpen_test_utils::get_common_db();
        let chain = gen_l2_chain(None, 5);

        for block in chain.clone() {
            db.as_ref()
                .l2_store()
                .as_ref()
                .put_block_data(block)
                .unwrap();
        }

        let block_ids: Vec<_> = chain.iter().map(|b| b.header().get_blockid()).collect();

        let mut last_finalized_block = Some(block_ids[0]);
        let new_finalized = Some(block_ids[4]);
        let finalized_blocks = get_finalized_blocks(
            new_finalized,
            db.l2_provider().as_ref(),
            &mut last_finalized_block,
        )
        .unwrap();

        let expected_finalized_blocks: Vec<_> = block_ids[1..=4].iter().rev().cloned().collect();

        assert_eq!(finalized_blocks, expected_finalized_blocks);
        assert_eq!(last_finalized_block, new_finalized);
    }
}
