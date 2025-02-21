use std::{thread, time};

// TODO: use local error type
use strata_consensus_logic::errors::Error;
use strata_eectl::{
    engine::{ExecEngineCtl, PayloadStatus},
    errors::EngineError,
    messages::{ExecPayloadData, PayloadEnv},
};
use strata_primitives::{
    buf::Buf32,
    l1::{L1BlockManifest, ProtocolOperation},
    params::{Params, RollupParams},
};
use strata_state::{
    block::{ExecSegment, L1Segment, L2BlockAccessory, L2BlockBundle},
    chain_state::Chainstate,
    exec_update::construct_ops_from_deposit_intents,
    header::L2BlockHeader,
    prelude::*,
    state_op::*,
};
use strata_storage::{L1BlockManager, NodeStorage};
use tracing::*;

/// Build contents for a new L2 block with the provided configuration.
/// Needs to be signed to be a valid L2Block.
// TODO use parent block chainstate
pub fn prepare_block(
    slot: u64,
    prev_block: L2BlockBundle,
    ts: u64,
    storage: &NodeStorage,
    engine: &impl ExecEngineCtl,
    params: &Params,
) -> Result<(L2BlockHeader, L2BlockBody, L2BlockAccessory), Error> {
    let prev_blkid = prev_block.header().get_blockid();
    debug!(%slot, %prev_blkid, "preparing block");
    let l1man = storage.l1();
    let chsman = storage.chainstate();

    let prev_global_sr = *prev_block.header().state_root();

    // Get the previous block's state
    // TODO make this get the prev block slot from somewhere more reliable in
    // case we skip slots
    let prev_slot = prev_block.header().blockidx();
    let prev_chstate = chsman
        .get_toplevel_chainstate_blocking(prev_slot)?
        .ok_or(Error::MissingBlockChainstate(prev_blkid))?;

    // Figure out the safe L1 blkid.
    // FIXME this is somewhat janky, should get it from the MMR
    let safe_l1_block_rec = prev_chstate.l1_view().safe_block();
    let safe_l1_blkid = strata_primitives::hash::sha256d(safe_l1_block_rec.buf());

    // TODO Pull data from CSM state that we've observed from L1, including new
    // headers or any headers needed to perform a reorg if necessary.
    let l1_seg = prepare_l1_segment(&prev_chstate, l1man.as_ref(), params.rollup())?;

    // Prepare the execution segment, which right now is just talking to the EVM
    // but will be more advanced later.
    let (exec_seg, block_acc) = prepare_exec_data(
        slot,
        ts,
        prev_blkid,
        prev_global_sr,
        &prev_chstate,
        safe_l1_blkid,
        engine,
        params.rollup(),
    )?;

    // Assemble the body and fake header.
    let body = L2BlockBody::new(l1_seg, exec_seg);
    let fake_header = L2BlockHeader::new(slot, ts, prev_blkid, &body, Buf32::zero());

    // Execute the block to compute the new state root, then assemble the real header.
    // TODO do something with the write batch?  to prepare it in the database?
    let (post_state, _wb) = compute_post_state(prev_chstate, &fake_header, &body, params)?;

    let new_state_root = post_state.compute_state_root();

    let header = L2BlockHeader::new(slot, ts, prev_blkid, &body, new_state_root);

    Ok((header, body, block_acc))
}

fn prepare_l1_segment(
    prev_chstate: &Chainstate,
    l1man: &L1BlockManager,
    params: &RollupParams,
) -> Result<L1Segment, Error> {
    // We aren't going to reorg, so we'll include blocks right up to the tip.
    let (cur_real_l1_height, _) = l1man
        .get_canonical_chain_tip()?
        .expect("blockasm: should have L1 blocks by now");
    let target_height = cur_real_l1_height.saturating_sub(params.l1_reorg_safe_depth as u64); // -1 to give some buffer for very short reorgs
    trace!(%target_height, "figuring out which blocks to include in L1 segment");

    // Check to see if there's actually no blocks in the queue.  In that case we can just give
    // everything we know about.
    let cur_safe_height = prev_chstate.l1_view().safe_height();
    let cur_next_exp_height = prev_chstate.l1_view().next_expected_height();

    // If there isn't any new blocks to pull then we just give nothing.
    if target_height <= cur_next_exp_height {
        return Ok(L1Segment::new_empty(cur_safe_height));
    }

    // This is much simpler than it was before because I'm removing the ability
    // to handle reorgs properly.  This is fine, we'll re-add it later when we
    // make the L1 scan proof stuff more sophisticated.
    let mut payloads = Vec::new();
    let mut is_epoch_final_block = {
        if prev_chstate.cur_epoch() == 0 {
            // no previous epoch, end epoch and send commitment immediately
            true
        } else {
            // check for previous epoch's checkpoint in referenced l1 blocks
            // end epoch once previous checkpoint is seen
            false
        }
    };

    for height in cur_next_exp_height..=target_height {
        let Some(rec) = try_fetch_manifest(height, l1man)? else {
            // If we are missing a record, then something is weird, but it would
            // still be safe to abort.
            warn!(%height, "missing expected L1 block during assembly");
            break;
        };

        for tx in rec.txs() {
            for op in tx.protocol_ops() {
                let ProtocolOperation::Checkpoint(_signed_checkpoint) = op else {
                    continue;
                };

                // TODO: check signed_checkpoint is the checkpoint we want
                // FIXME: we really need to do this
                is_epoch_final_block = true;
            }
        }

        payloads.push(rec);

        if is_epoch_final_block {
            // if epoch = 0, include first seen l1 block and create checkpoint
            // if epoch > 0, include till first seen block with correct checkpoint
            break;
        }
    }

    if !payloads.is_empty() {
        debug!(n = %payloads.len(), "have new L1 blocks to provide");
    }

    if is_epoch_final_block {
        let new_height = cur_safe_height + payloads.len() as u64;
        Ok(L1Segment::new(new_height, payloads))
    } else {
        Ok(L1Segment::new(cur_safe_height, Vec::new()))
    }
}

#[allow(unused)]
fn fetch_manifest(h: u64, l1man: &L1BlockManager) -> Result<L1BlockManifest, Error> {
    try_fetch_manifest(h, l1man)?.ok_or(Error::MissingL1BlockHeight(h))
}

fn try_fetch_manifest(h: u64, l1man: &L1BlockManager) -> Result<Option<L1BlockManifest>, Error> {
    Ok(l1man.get_block_manifest_at_height(h)?)
}

/// Prepares the execution segment for the block.
#[allow(clippy::too_many_arguments)]
fn prepare_exec_data<E: ExecEngineCtl>(
    _slot: u64,
    timestamp: u64,
    prev_l2_blkid: L2BlockId,
    _prev_global_sr: Buf32,
    prev_chstate: &Chainstate,
    safe_l1_block: Buf32,
    engine: &E,
    params: &RollupParams,
) -> Result<(ExecSegment, L2BlockAccessory), Error> {
    trace!("preparing exec payload");

    // Start preparing the EL payload.

    // construct el_ops by looking at chainstate
    let pending_deposits = prev_chstate.exec_env_state().pending_deposits();
    let el_ops = construct_ops_from_deposit_intents(pending_deposits, params.max_deposits_in_block);
    let payload_env = PayloadEnv::new(timestamp, prev_l2_blkid, safe_l1_block, el_ops);

    let key = engine.prepare_payload(payload_env)?;
    trace!("submitted EL payload job, waiting for completion");

    // Wait 2 seconds for the block to be finished.
    // TODO Pull data from state about the new safe L1 hash, prev state roots,
    // etc. to assemble the payload env for this block.
    let wait = time::Duration::from_millis(100);
    let timeout = time::Duration::from_millis(3000);
    let Some(payload_data) = poll_status_loop(key, engine, wait, timeout)? else {
        return Err(Error::BlockAssemblyTimedOut);
    };
    trace!("finished EL payload job");

    // Reassemble it into an exec update.
    let exec_update = payload_data.exec_update().clone();
    let _applied_ops = payload_data.ops();
    let exec_seg = ExecSegment::new(exec_update);

    // And the accessory.
    let acc = L2BlockAccessory::new(payload_data.accessory_data().to_vec());

    Ok((exec_seg, acc))
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
        trace!(%job, "polling engine for completed payload");
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

// TODO when we build the "block executor" logic we should shift this out
fn compute_post_state(
    prev_chstate: Chainstate,
    header: &impl L2Header,
    body: &L2BlockBody,
    params: &Params,
) -> Result<(Chainstate, WriteBatch), Error> {
    let mut state_cache = StateCache::new(prev_chstate);
    strata_chaintsn::transition::process_block(&mut state_cache, header, body, params.rollup())?;
    let (post_state, wb) = state_cache.finalize();
    Ok((post_state, wb))
}
