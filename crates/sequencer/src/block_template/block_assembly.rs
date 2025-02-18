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
    l1::{DepositUpdateTx, L1HeaderPayload, L1HeaderRecord, ProtocolOperation},
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

/// Max number of L1 block entries to include per L2Block.
/// This is relevant when sequencer starts at L1 height >> genesis height
/// to prevent L2Block from becoming very large in size.
const MAX_L1_ENTRIES_PER_BLOCK: usize = 100;

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

    // Figure out the save L1 blkid.
    // FIXME this is somewhat janky, should get it from the MMR
    let safe_l1_block_rec = prev_chstate.l1_view().safe_block();
    let safe_l1_blkid = strata_primitives::hash::sha256d(safe_l1_block_rec.buf());

    // TODO Pull data from CSM state that we've observed from L1, including new
    // headers or any headers needed to perform a reorg if necessary.
    let l1_seg = prepare_l1_segment(
        &prev_chstate,
        l1man.as_ref(),
        MAX_L1_ENTRIES_PER_BLOCK,
        params.rollup(),
    )?;

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
    max_l1_entries: usize,
    _params: &RollupParams,
) -> Result<L1Segment, Error> {
    // We aren't going to reorg, so we'll include blocks right up to the tip.
    let cur_real_l1_height = l1man
        .get_chain_tip()?
        .expect("blockasm: should have L1 blocks by now");
    let target_height = cur_real_l1_height - 1; // -1 to give some buffer
    trace!(%target_height, "figuring out which blocks to include in L1 segment");

    // Check to see if there's actually no blocks in the queue.  In that case we can just give
    // everything we know about.
    let _maturation_queue_size = prev_chstate.l1_view().maturation_queue().len();
    let cur_state_l1_height = prev_chstate.l1_view().tip_height();

    // If there isn't any new blocks to pull then we just give nothing.
    if target_height <= cur_state_l1_height {
        return Ok(L1Segment::new_empty());
    }

    // This is much simpler than it was before because I'm removing the ability
    // to handle reorgs properly.  This is fine, we'll readd it later when we
    // make the L1 scan proof stuff more sophisticated.
    let mut payloads = Vec::new();
    for off in 1..max_l1_entries {
        let height = cur_state_l1_height + off as u64;

        // If we get to the tip then we should break here.
        if height > target_height {
            break;
        }

        let Some(rec) = try_fetch_header_record(height, l1man)? else {
            // If we are missing a record, then something is weird, but it would
            // still be safe to abort.
            warn!(%height, "missing expected L1 block during assembly");
            break;
        };

        let deposits = fetch_deposit_update_txs(height, l1man)?;

        payloads.push(
            L1HeaderPayload::new(height, rec)
                .with_deposit_update_txs(deposits)
                .build(),
        );
    }

    if !payloads.is_empty() {
        debug!(n = %payloads.len(), "have new L1 blocks to provide");
    }

    Ok(L1Segment::new(payloads))
}

fn fetch_header_record(h: u64, l1man: &L1BlockManager) -> Result<L1HeaderRecord, Error> {
    try_fetch_header_record(h, l1man)?.ok_or(Error::MissingL1BlockHeight(h))
}

fn try_fetch_header_record(
    h: u64,
    l1man: &L1BlockManager,
) -> Result<Option<L1HeaderRecord>, Error> {
    let Some(mf) = l1man.get_block_manifest(h)? else {
        return Ok(None);
    };

    // TODO need to include tx root proof we can verify
    Ok(Some(L1HeaderRecord::create_from_serialized_header(
        mf.header().to_vec(),
        mf.txs_root(),
    )))
}

fn fetch_deposit_update_txs(h: u64, l1man: &L1BlockManager) -> Result<Vec<DepositUpdateTx>, Error> {
    let relevant_tx_ref = l1man
        .get_block_txs(h)?
        .ok_or(Error::MissingL1BlockHeight(h))?;

    let mut deposit_update_txs = Vec::new();
    for tx_ref in relevant_tx_ref {
        let tx = l1man.get_tx(tx_ref)?.ok_or(Error::MissingL1Tx)?;

        for op in tx.protocol_ops() {
            if let ProtocolOperation::Deposit(_dep) = op {
                deposit_update_txs.push(DepositUpdateTx::new(tx.clone(), tx_ref.position()))
            }
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

// TODO do we want to do this here?
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
