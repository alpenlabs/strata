//! Impl logic for the block assembly duties.
#![allow(unused)]

use std::{sync::Arc, thread, time};

use bitcoin::Transaction;
use strata_crypto::sign_schnorr_sig;
use strata_db::traits::{
    ChainstateDatabase, ClientStateDatabase, Database, L1Database, L2BlockDatabase,
};
use strata_eectl::{
    engine::{ExecEngineCtl, PayloadStatus},
    errors::EngineError,
    messages::{ExecPayloadData, PayloadEnv},
};
use strata_primitives::{
    buf::{Buf32, Buf64},
    params::{Params, RollupParams},
};
use strata_state::{
    block::{ExecSegment, L1Segment, L2BlockAccessory, L2BlockBundle},
    bridge_ops::DepositIntent,
    chain_state::Chainstate,
    client_state::{ClientState, LocalL1State},
    exec_update::{
        construct_ops_from_deposit_intents, ELDepositData, ExecUpdate, Op, UpdateOutput,
    },
    header::L2BlockHeader,
    l1::{DepositUpdateTx, L1HeaderPayload, L1HeaderRecord},
    prelude::*,
    state_op::*,
    tx::ProtocolOperation::Deposit,
};
use tracing::*;

use super::types::*;
use crate::errors::Error;

/// Max number of L1 block entries to include per L2Block.
/// This is relevant when sequencer starts at L1 height >> genesis height
/// to prevent L2Block from becoming very large in size.
const MAX_L1_ENTRIES_PER_BLOCK: usize = 100;

/// Signs and stores a block in the database.  Does not submit it to the
/// forkchoice manager.
// TODO pass in the CSM state we're using to assemble this
pub(super) fn sign_and_store_block<D: Database, E: ExecEngineCtl>(
    slot: u64,
    prev_block_id: L2BlockId,
    l1_state: &LocalL1State,
    ik: &IdentityKey,
    database: &D,
    engine: &E,
    params: &Arc<Params>,
) -> Result<Option<(L2BlockId, L2Block)>, Error> {
    debug!("preparing block");
    let l1_db = database.l1_db();
    let l2_db = database.l2_db();
    let cs_db = database.client_state_db();
    let chs_db = database.chain_state_db();

    // Check the block we were supposed to build isn't already in the database,
    // if so then just republish that.  This checks that there just if we have a
    // block at that height, which for now is the same thing.
    let blocks_at_slot = l2_db.get_blocks_at_height(slot)?;
    if !blocks_at_slot.is_empty() {
        // FIXME Should we be more verbose about this?
        warn!(%slot, "was turn to propose block, but found block in database already");
        return Ok(None);
    }

    // Figure out some general data about the slot and the previous state.
    let ts = now_millis();

    let prev_block = l2_db
        .get_block_data(prev_block_id)?
        .ok_or(Error::MissingL2Block(prev_block_id))?;

    // TODO: get from rollup config
    let block_time = params.rollup().block_time;
    let target_ts = prev_block.block().header().timestamp() + block_time;
    let current_ts = now_millis();

    // If it's too early to produce the block, wait a bit.
    if current_ts < target_ts {
        let sleep_dur = target_ts - current_ts;
        trace!(%current_ts, %target_ts, sleep_dur, "too early, waiting to produce block");
        thread::sleep(time::Duration::from_millis(sleep_dur));
    }

    let ts = now_millis();

    let prev_global_sr = *prev_block.header().state_root();

    // Get the previous block's state
    // TODO make this get the prev block slot from somewhere more reliable in
    // case we skip slots
    let prev_slot = prev_block.header().blockidx();
    let prev_chstate = chs_db
        .get_toplevel_state(prev_slot)?
        .ok_or(Error::MissingBlockChainstate(prev_block_id))?;

    // Figure out the save L1 blkid.
    // FIXME this is somewhat janky, should get it from the MMR
    let safe_l1_block_rec = prev_chstate.l1_view().safe_block();
    let safe_l1_blkid = strata_primitives::hash::sha256d(safe_l1_block_rec.buf());

    // TODO Pull data from CSM state that we've observed from L1, including new
    // headers or any headers needed to perform a reorg if necessary.
    let l1_seg = prepare_l1_segment(
        l1_state,
        &prev_chstate,
        l1_db.as_ref(),
        MAX_L1_ENTRIES_PER_BLOCK,
        params.rollup(),
    )?;

    // Prepare the execution segment, which right now is just talking to the EVM
    // but will be more advanced later.
    let (exec_seg, block_acc) = prepare_exec_data(
        slot,
        ts,
        prev_block_id,
        prev_global_sr,
        &prev_chstate,
        safe_l1_blkid,
        engine,
        params.rollup(),
    )?;

    // Assemble the body and fake header.
    let body = L2BlockBody::new(l1_seg, exec_seg);
    let fake_header = L2BlockHeader::new(slot, ts, prev_block_id, &body, Buf32::zero());

    // Execute the block to compute the new state root, then assemble the real header.
    // TODO do something with the write batch?  to prepare it in the database?
    let (post_state, wb) = compute_post_state(prev_chstate, &fake_header, &body, params)?;

    let new_state_root = post_state.compute_state_root();

    let header = L2BlockHeader::new(slot, ts, prev_block_id, &body, new_state_root);
    let header_sig = sign_header(&header, ik);
    let signed_header = SignedL2BlockHeader::new(header, header_sig);

    let blkid = signed_header.get_blockid();
    let final_block = L2Block::new(signed_header, body);
    let final_bundle = L2BlockBundle::new(final_block.clone(), block_acc);
    info!(?blkid, "finished building new block");

    // Store the block in the database.
    let l2_db = database.l2_db();
    l2_db.put_block_data(final_bundle)?;
    debug!(?blkid, "wrote block to datastore");

    // TODO should we actually return the bundle here?
    Ok(Some((blkid, final_block)))
}

fn prepare_l1_segment(
    local_l1_state: &LocalL1State,
    prev_chstate: &Chainstate,
    l1_db: &impl L1Database,
    max_l1_entries: usize,
    params: &RollupParams,
) -> Result<L1Segment, Error> {
    let unacc_blocks = local_l1_state.unacc_blocks_iter().collect::<Vec<_>>();
    trace!(unacc_blocks = %unacc_blocks.len(), "figuring out which blocks to include in L1 segment");

    // Check to see if there's actually no blocks in the queue.  In that case we can just give
    // everything we know about.
    let maturation_queue_size = prev_chstate.l1_view().maturation_queue().len();
    if maturation_queue_size == 0 {
        let mut payloads = Vec::new();
        for (h, _b) in unacc_blocks.iter().take(max_l1_entries) {
            let rec = load_header_record(*h, l1_db)?;
            let deposit_update_tx = fetch_deposit_update_txs(*h, l1_db)?;
            payloads.push(
                L1HeaderPayload::new(*h, rec)
                    .with_deposit_update_txs(deposit_update_tx)
                    .build(),
            );
        }

        debug!(n = %payloads.len(), "filling in empty queue with fresh L1 payloads");
        return Ok(L1Segment::new(payloads));
    }

    let maturing_blocks = prev_chstate
        .l1_view()
        .maturation_queue()
        .iter_entries()
        .map(|(h, e)| (h, e.blkid()))
        .collect::<Vec<_>>();

    // FIXME this is not comparing the proof of work, it's just looking at the chain lengths, this
    // is almost the same thing, but might break in the case of a difficulty adjustment taking place
    // at a reorg exactly on the transition block
    trace!("computing pivot");

    let Some((pivot_h, _pivot_id)) = find_pivot_block_height(&unacc_blocks, &maturing_blocks)
    else {
        // Then we're really screwed.
        error!("can't determine shared block to insert new maturing blocks");
        return Err(Error::BlockAssemblyL1SegmentUndetermined);
    };

    // Compute the offset in the unaccepted list for the blocks we want to use.
    let unacc_fresh_offset = (pivot_h - local_l1_state.buried_l1_height()) as usize + 1;

    let fresh_blocks = &unacc_blocks
        .iter()
        .skip(unacc_fresh_offset)
        .take(max_l1_entries)
        .collect::<Vec<_>>();

    // Load the blocks.
    let mut payloads = Vec::new();
    for (h, _b) in fresh_blocks {
        let rec = load_header_record(*h, l1_db)?;
        let deposit_update_tx = fetch_deposit_update_txs(*h, l1_db)?;
        payloads.push(
            L1HeaderPayload::new(*h, rec)
                .with_deposit_update_txs(deposit_update_tx)
                .build(),
        );
    }

    if !payloads.is_empty() {
        debug!(n = %payloads.len(), "have new L1 blocks to provide");
    }

    Ok(L1Segment::new(payloads))
}

fn load_header_record(h: u64, l1_db: &impl L1Database) -> Result<L1HeaderRecord, Error> {
    let mf = l1_db
        .get_block_manifest(h)?
        .ok_or(Error::MissingL1BlockHeight(h))?;
    // TODO need to include tx root proof we can verify
    Ok(L1HeaderRecord::create_from_serialized_header(
        mf.header().to_vec(),
        mf.txs_root(),
    ))
}

fn fetch_deposit_update_txs(
    h: u64,
    l1_db: &impl L1Database,
) -> Result<Vec<DepositUpdateTx>, Error> {
    let relevant_tx_ref = l1_db
        .get_block_txs(h)?
        .ok_or(Error::MissingL1BlockHeight(h))?;

    let mut deposit_update_txs = Vec::new();
    for tx_ref in relevant_tx_ref {
        let tx = l1_db.get_tx(tx_ref)?.ok_or(Error::MissingL1Tx)?;

        if let Deposit(dep) = tx.protocol_operation() {
            deposit_update_txs.push(DepositUpdateTx::new(tx, tx_ref.position()));
        }
    }

    Ok(deposit_update_txs)
}

/// Takes two partially-overlapping lists of block indexes and IDs and returns a
/// ref to the last index and ID they share, if any.
fn find_pivot_block_height<'c>(
    a: &'c [(u64, &'c L1BlockId)],
    b: &'c [(u64, &'c L1BlockId)],
) -> Option<(u64, &'c L1BlockId)> {
    if a.is_empty() || b.is_empty() {
        return None;
    }

    let a_start = a[0].0 as i64;
    let b_start = b[0].0 as i64;
    let a_end = a_start + (a.len() as i64);
    let b_end = b_start + (b.len() as i64);

    #[cfg(test)]
    eprintln!("ranges {a_start}..{a_end}, {b_start}..{b_end}");

    // Check if they're actually overlapping.
    if !(a_start < b_end && b_start < a_end) {
        return None;
    }

    // Depending on how the windows overlap at the start we figure out the offsets for how we're
    // iterating here.
    let (a_off, b_off) = match b_start - a_start {
        0 => (0, 0),
        n if n > 0 => (n, 0),
        n if n < 0 => (0, -n),
        _ => unreachable!(),
    };

    #[cfg(test)]
    eprintln!("offsets {a_off} {b_off}");

    // Sanity checks.
    assert!(a_off >= 0);
    assert!(b_off >= 0);

    let overlap_len = i64::min(a_end, b_end) - i64::max(a_start, b_start);

    #[cfg(test)]
    eprintln!("overlap {overlap_len}");

    // Now iterate over the overlap range and return if it makes sense.
    let mut last = None;
    for i in 0..overlap_len {
        let (ai, aid) = a[(i + a_off) as usize];
        let (bi, bid) = b[(i + b_off) as usize];

        #[cfg(test)]
        {
            eprintln!("Ai {ai} {aid}");
            eprintln!("Bi {bi} {bid}");
        }

        assert_eq!(ai, bi); // Sanity check.

        if aid != bid {
            break;
        }

        last = Some((ai, aid));
    }

    last
}

/// Prepares the execution segment for the block.
#[allow(clippy::too_many_arguments)]
fn prepare_exec_data<E: ExecEngineCtl>(
    slot: u64,
    timestamp: u64,
    prev_l2_blkid: L2BlockId,
    prev_global_sr: Buf32,
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
    let applied_ops = payload_data.ops();
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

// TODO do we want to do this here?
fn compute_post_state(
    prev_chstate: Chainstate,
    header: &impl L2Header,
    body: &L2BlockBody,
    params: &Arc<Params>,
) -> Result<(Chainstate, WriteBatch), Error> {
    let mut state_cache = StateCache::new(prev_chstate);
    strata_chaintsn::transition::process_block(&mut state_cache, header, body, params.rollup())?;
    let (post_state, wb) = state_cache.finalize();
    Ok((post_state, wb))
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

#[cfg(test)]
mod tests {
    // TODO to improve these tests, they could use a bit more randomization of lengths and offsets

    use strata_state::l1::L1BlockId;
    use strata_test_utils::ArbitraryGenerator;

    use super::find_pivot_block_height;

    #[test]
    fn test_find_pivot_noop() {
        let mut ag = ArbitraryGenerator::new_with_size(1 << 12);

        let blkids: [L1BlockId; 10] = ag.generate();
        eprintln!("{blkids:#?}");

        let blocks = blkids
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let max_height = blocks.last().unwrap().0;
        let max_id = blocks.last().unwrap().1;

        let (shared_h, shared_id) =
            find_pivot_block_height(&blocks, &blocks).expect("test: find pivot");

        assert_eq!(shared_h, max_height);
        assert_eq!(shared_id, max_id);
    }

    #[test]
    fn test_find_pivot_noop_offset() {
        let mut ag = ArbitraryGenerator::new_with_size(1 << 12);

        let blkids: [L1BlockId; 10] = ag.generate();
        eprintln!("{blkids:#?}");

        let blocks = blkids
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let max_height = blocks[7].0;
        let max_id = blocks[7].1;

        let (shared_h, shared_id) =
            find_pivot_block_height(&blocks[..8], &blocks[2..]).expect("test: find pivot");

        assert_eq!(shared_h, max_height);
        assert_eq!(shared_id, max_id);
    }

    #[test]
    fn test_find_pivot_simple_extend() {
        let mut ag = ArbitraryGenerator::new_with_size(1 << 12);

        let blkids1: [L1BlockId; 10] = ag.generate();
        let mut blkids2 = Vec::from(blkids1);
        blkids2.push(ag.generate());
        blkids2.push(ag.generate());
        eprintln!("{blkids1:#?}");

        let blocks1 = blkids1
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let blocks2 = blkids2
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let max_height = blocks1.last().unwrap().0;
        let max_id = blocks1.last().unwrap().1;

        let (shared_h, shared_id) =
            find_pivot_block_height(&blocks1, &blocks2).expect("test: find pivot");

        assert_eq!(shared_h, max_height);
        assert_eq!(shared_id, max_id);
    }

    #[test]
    fn test_find_pivot_typical_reorg() {
        let mut ag = ArbitraryGenerator::new_with_size(1 << 16);

        let mut blkids1: Vec<L1BlockId> = Vec::new();
        for _ in 0..10 {
            blkids1.push(ag.generate());
        }

        let mut blkids2 = blkids1.clone();
        blkids2.pop();
        blkids2.push(ag.generate());
        blkids2.push(ag.generate());

        let blocks1 = blkids1
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let blocks2 = blkids2
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let max_height = blocks1[blocks1.len() - 2].0;
        let max_id = blocks1[blocks1.len() - 2].1;

        // Also using a pretty deep offset here.
        let (shared_h, shared_id) =
            find_pivot_block_height(&blocks1, &blocks2[5..]).expect("test: find pivot");

        assert_eq!(shared_h, max_height);
        assert_eq!(shared_id, max_id);
    }

    #[test]
    fn test_find_pivot_cur_shorter_reorg() {
        let mut ag = ArbitraryGenerator::new_with_size(1 << 16);

        let mut blkids1: Vec<L1BlockId> = Vec::new();
        for _ in 0..10 {
            blkids1.push(ag.generate());
        }

        let mut blkids2 = blkids1.clone();
        blkids2.pop();
        blkids2.pop();
        blkids2.pop();
        blkids2.pop();
        let len = blkids2.len();
        blkids2.push(ag.generate());
        blkids2.push(ag.generate());

        let blocks1 = blkids1
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let blocks2 = blkids2
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let max_height = blocks2[len - 1].0;
        let max_id = blocks2[len - 1].1;

        // Also using a pretty deep offset here.
        let (shared_h, shared_id) =
            find_pivot_block_height(&blocks1, &blocks2[5..]).expect("test: find pivot");

        assert_eq!(shared_h, max_height);
        assert_eq!(shared_id, max_id);
    }

    #[test]
    fn test_find_pivot_disjoint() {
        let mut ag = ArbitraryGenerator::new_with_size(1 << 16);

        let mut blkids1: Vec<L1BlockId> = Vec::new();
        for _ in 0..10 {
            blkids1.push(ag.generate());
        }

        let mut blkids2: Vec<L1BlockId> = Vec::new();
        for _ in 0..10 {
            blkids2.push(ag.generate());
        }

        let blocks1 = blkids1
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let blocks2 = blkids2
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u64 + 8, b))
            .collect::<Vec<_>>();

        let res = find_pivot_block_height(&blocks1[2..], &blocks2[..3]);
        assert!(res.is_none());
    }
}
