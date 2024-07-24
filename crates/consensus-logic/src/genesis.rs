use std::{sync::Arc, thread, time::Duration};

use tracing::*;

use alpen_vertex_db::{
    errors::DbError,
    traits::{
        ChainstateStore, ClientStateProvider, ClientStateStore, Database, L1DataProvider,
        L2DataProvider, L2DataStore,
    },
};
use alpen_vertex_primitives::{
    buf::{Buf32, Buf64},
    l1::L1BlockManifest,
    params::Params,
};
use alpen_vertex_state::{
    block::{ExecSegment, L1Segment},
    chain_state::ChainState,
    client_state::ClientState,
    exec_env::ExecEnvState,
    exec_update::{ExecUpdate, UpdateInput, UpdateOutput},
    header::L2BlockHeader,
    l1::{L1HeaderRecord, L1ViewState},
    prelude::*,
};

const MAX_HORIZON_POLL_RETRIES: u64 = 10;
const MAX_HORIZON_POLL_INTERVAL: u64 = 1000;

fn poll_horizon_l1_block<D: Database>(
    l1_prov: Arc<D::L1Prov>,
    horizon_blk_height: u64,
) -> anyhow::Result<L1BlockManifest> {
    // Fetch the horizon L1 block to construct the genesis L1 segment.

    let mut retries = 0;
    loop {
        if let Some(mf) = l1_prov.get_block_manifest(horizon_blk_height)? {
            return Ok(mf);
        }
        thread::sleep(Duration::from_millis(MAX_HORIZON_POLL_INTERVAL));
        if retries > MAX_HORIZON_POLL_RETRIES {
            break;
        }
        retries += 1;
    }
    return Err(anyhow::anyhow!(
        "Max retries exceeded for polling horizon l1 block"
    ));
}

/// Inserts approprate records into the database to prepare it for syncing the
/// rollup.  Requires that the horizon block header is present in the database.
pub fn init_genesis_states<D: Database>(params: &Params, database: &D) -> anyhow::Result<()> {
    debug!("preparing database genesis state!");

    let horizon_blk_height = params.rollup.l1_start_block_height;

    // Create a dummy exec state that we can build the rest of the genesis block
    // around and insert into the genesis state.
    // TODO this might need to talk to the EL to do the genesus setup *properly*
    let geui = UpdateInput::new(0, Buf32::zero(), Vec::new());
    let gees = ExecEnvState::from_base_input(geui.clone(), Buf32::zero());
    let genesis_ee_state = Buf32::zero();
    let geu = ExecUpdate::new(geui.clone(), UpdateOutput::new_from_state(genesis_ee_state));

    // Build the genesis block and genesis consensus states.
    let gblock = make_genesis_block(params, geu);
    let genesis_blkid = gblock.header().get_blockid();
    info!(?genesis_blkid, "created genesis block");

    let l1_prov = database.l1_provider();
    let horizon_blkmf = poll_horizon_l1_block::<D>(l1_prov.clone(), horizon_blk_height)?;

    let horizon_blk_rec = L1HeaderRecord::from(&horizon_blkmf);
    let l1vs = L1ViewState::new_at_horizon(horizon_blk_height, horizon_blk_rec);

    let gchstate = ChainState::from_genesis(genesis_blkid, l1vs, gees);
    let gclstate = make_genesis_client_state(&gblock, &gchstate, params);

    // Now insert things into the database.
    let l2store = database.l2_store();
    let cs_store = database.client_state_store();
    let chs_store = database.chainstate_store();
    l2store.put_block_data(gblock)?;
    chs_store.write_genesis_state(&gchstate)?;
    cs_store.write_client_state_checkpoint(0, gclstate)?;

    info!("finished genesis insertions");
    Ok(())
}

fn make_genesis_block(params: &Params, genesis_update: ExecUpdate) -> L2Block {
    // This has to be empty since everyone should have an unambiguous view of the genesis block.
    let l1_seg = L1Segment::new_empty();

    // TODO this is a total stub, we have to fill it in with something
    let exec_seg = ExecSegment::new(genesis_update);

    let body = L2BlockBody::new(l1_seg, exec_seg);

    // Assemble the genesis header core, pulling in data from whatever
    // sources we need.
    // FIXME this isn't the right timestamp to start the blockchain, this should
    // definitely be pulled from the database or the rollup parameters maybe
    let genesis_ts = params.rollup().l1_start_block_height;
    let zero_blkid = L2BlockId::from(Buf32::zero());
    let genesis_sr = Buf32::zero();
    let header = L2BlockHeader::new(0, genesis_ts, zero_blkid, &body, genesis_sr);
    let signed_genesis_header = SignedL2BlockHeader::new(header, Buf64::zero());
    L2Block::new(signed_genesis_header, body)
}

fn make_genesis_client_state(
    _gblock: &L2Block,
    gchstate: &ChainState,
    params: &Params,
) -> ClientState {
    // TODO this might rework some more things as we include the genesis block
    ClientState::from_genesis(gchstate, params.rollup().l1_start_block_height)
}

/// Check if the database needs to have genesis done to it.
pub fn check_needs_genesis<D: Database>(database: &D) -> anyhow::Result<bool> {
    let cs_prov = database.client_state_provider();

    // Check if we've written the genesis state checkpoint.  This should be the
    // only check we have to do, but it's possible we're in an inconsistent
    // state so we do perform others
    if cs_prov.get_state_checkpoint(0)?.is_none() {
        return Ok(true);
    }

    // Check if we've written the genesis state checkpoint.  These we perform a
    // bit more carefully and check errors more granularly.
    match cs_prov.get_last_checkpoint_idx() {
        Ok(_) => {}
        Err(DbError::NotBootstrapped) => return Ok(true),

        // TODO should we return an error here or skip?
        Err(e) => return Err(e.into()),
    }

    let l2prov = database.l2_provider();

    // Check if there's any genesis block written.
    match l2prov.get_blocks_at_height(0) {
        Ok(blkids) => {
            if blkids.is_empty() {
                return Ok(true);
            }
        }

        Err(DbError::NotBootstrapped) => return Ok(true),

        // Again, how should we handle this?
        Err(e) => return Err(e.into()),
    }

    Ok(false)
}
