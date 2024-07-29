//! Impl logic for the block assembly duties.

use std::sync::Arc;
use std::thread;
use std::time;

use tracing::*;

use alpen_express_db::traits::{
    ChainstateProvider, ClientStateProvider, Database, L2DataProvider, L2DataStore,
};
use alpen_express_evmctl::engine::{ExecEngineCtl, PayloadStatus};
use alpen_express_evmctl::errors::EngineError;
use alpen_express_evmctl::messages::{ExecPayloadData, PayloadEnv};
use alpen_express_primitives::buf::{Buf32, Buf64};
use alpen_express_primitives::params::Params;
use alpen_express_state::block::L2BlockAccessory;
use alpen_express_state::block::L2BlockBundle;
use alpen_express_state::block::{ExecSegment, L1Segment};
use alpen_express_state::chain_state::ChainState;
use alpen_express_state::client_state::ClientState;
use alpen_express_state::exec_update::{ExecUpdate, UpdateInput, UpdateOutput};
use alpen_express_state::header::L2BlockHeader;
use alpen_express_state::prelude::*;

use super::types::IdentityKey;
use crate::credential::sign_schnorr_sig;
use crate::errors::Error;

/// Signs and stores a block in the database.  Does not submit it to the
/// forkchoice manager.
// TODO pass in the CSM state we're using to assemble this
pub(super) fn sign_and_store_block<D: Database, E: ExecEngineCtl>(
    slot: u64,
    ik: &IdentityKey,
    database: &D,
    engine: &E,
    params: &Arc<Params>,
) -> Result<Option<(L2BlockId, L2Block)>, Error> {
    debug!("preparing block");
    let l2_prov = database.l2_provider();
    let cs_prov = database.client_state_provider();
    let chs_prov = database.chainstate_provider();

    // Check the block we were supposed to build isn't already in the database,
    // if so then just republish that.  This checks that there just if we have a
    // block at that height, which for now is the same thing.
    let blocks_at_slot = l2_prov.get_blocks_at_height(slot)?;
    if !blocks_at_slot.is_empty() {
        // FIXME Should we be more verbose about this?
        warn!(%slot, "was turn to propose block, but found block in database already");
        return Ok(None);
    }

    // TODO get the consensus state this duty was created in response to and
    // pull out the current tip block from it
    // XXX this is really bad as-is, see comment in worker.rs in `perform_duty`
    // FIXME this isn't what this is for, it only works because we're
    // checkpointing on every state right now
    let ckpt_idx = cs_prov.get_last_checkpoint_idx()?;
    let last_cstate = cs_prov
        .get_state_checkpoint(ckpt_idx)?
        .expect("dutyexec: get state checkpoint");

    // Figure out some general data about the slot and the previous state.
    let ts = now_millis();

    let safe_l1_block = Buf32::zero(); // TODO

    let Some(last_ss) = last_cstate.sync() else {
        return Ok(None);
    };

    let prev_block_id = *last_ss.chain_tip_blkid();
    let prev_block = l2_prov
        .get_block_data(prev_block_id)?
        .ok_or(Error::MissingL2Block(prev_block_id))?;

    // TODO: get from rollup config
    let block_time = params.rollup().block_time;
    let target_ts = prev_block.block().header().timestamp() + block_time;
    let current_ts = now_millis();

    if current_ts < target_ts {
        thread::sleep(time::Duration::from_millis(target_ts - current_ts));
    }

    let prev_global_sr = *prev_block.header().state_root();

    // Get the previous block's state
    // TODO make this get the prev block slot from somewhere more reliable in
    // case we skip slots
    let prev_slot = prev_block.header().blockidx();
    let prev_chstate = chs_prov
        .get_toplevel_state(prev_slot)?
        .ok_or(Error::MissingBlockChainstate(prev_block_id))?;

    // TODO Pull data from CSM state that we've observed from L1, including new
    // headers or any headers needed to perform a reorg if necessary.
    let l1_seg = prepare_l1_segment(&last_cstate, &prev_chstate)?;

    // Prepare the execution segment, which right now is just talking to the EVM
    // but will be more advanced later.
    let exec_seg = prepare_exec_segment(slot, ts, prev_global_sr, safe_l1_block, engine)?;

    // Assemble the body and the header core.
    let body = L2BlockBody::new(l1_seg, exec_seg);
    let state_root = Buf32::zero(); // TODO compute this from the different parts
    let header = L2BlockHeader::new(slot, ts, prev_block_id, &body, state_root);
    let header_sig = sign_header(&header, ik);
    let signed_header = SignedL2BlockHeader::new(header, header_sig);
    let blkid = signed_header.get_blockid();
    let final_block = L2Block::new(signed_header, body);

    // FIXME this is kind bodged together, it should be pulling more data in from the engine
    let block_acc = L2BlockAccessory::new(Vec::new());
    let final_bundle = L2BlockBundle::new(final_block.clone(), block_acc);
    info!(?blkid, "finished building new block");

    // Store the block in the database.
    let l2store = database.l2_store();
    l2store.put_block_data(final_bundle)?;
    debug!(?blkid, "wrote block to datastore");

    Ok(Some((blkid, final_block)))
}

fn prepare_l1_segment(cstate: &ClientState, prev_chstate: &ChainState) -> Result<L1Segment, Error> {
    Ok(L1Segment::new_empty())
}

/// Prepares the execution segment for the block.
fn prepare_exec_segment<E: ExecEngineCtl>(
    slot: u64,
    timestamp: u64,
    prev_global_sr: Buf32,
    safe_l1_block: Buf32,
    engine: &E,
) -> Result<ExecSegment, Error> {
    // Start preparing the EL payload.
    // FIXME this isn't right, we should be getting the state
    let prev_l2_block_id = L2BlockId::from(Buf32::zero());
    let payload_env = PayloadEnv::new(timestamp, prev_l2_block_id, safe_l1_block, Vec::new());
    let key = engine.prepare_payload(payload_env)?;
    trace!("submitted EL payload job, waiting for completion");

    // Wait 2 seconds for the block to be finished.
    // TODO Pull data from state about the new safe L1 hash, prev state roots,
    // etc. to assemble the payload env for this block.
    let wait = time::Duration::from_millis(100);
    let timeout = time::Duration::from_millis(3000);
    let Some(payload_data) = poll_status_loop(key, engine, wait, timeout)? else {
        // TODO better error message
        return Err(Error::Other("EL block assembly timed out".to_owned()));
    };
    trace!("finished EL payload job");

    // TODO correctly assemble the exec segment, since this is bodging out how
    // the inputs/outputs should be structured
    // FIXME this was broken in a rebase
    let eui = UpdateInput::new(slot, Buf32::zero(), payload_data.accessory_data().to_vec());
    let exec_update = ExecUpdate::new(eui, UpdateOutput::new_from_state(Buf32::zero()));

    Ok(ExecSegment::new(exec_update))
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
