use tracing::*;

use alpen_vertex_db::{
    errors::DbError,
    traits::{
        ChainstateStore, ClientStateProvider, ClientStateStore, Database, L2DataProvider,
        L2DataStore,
    },
};
use alpen_vertex_primitives::{
    buf::{Buf32, Buf64},
    params::Params,
};
use alpen_vertex_state::prelude::*;
use alpen_vertex_state::{
    block::{ExecSegment, L1Segment},
    block_template,
    chain_state::ChainState,
    client_state::ClientState,
};

/// Inserts approprate records into the database to prepare it for syncing the rollup.
pub fn init_genesis_states<D: Database>(params: &Params, database: &D) -> anyhow::Result<()> {
    debug!("preparing database genesis state!");

    // Build the genesis block and genesis consensus states.
    let gblock = make_genesis_block(params);
    let genesis_blkid = gblock.header().get_blockid();
    trace!(?genesis_blkid, "created genesis block");
    let gchstate = ChainState::from_genesis_blkid(genesis_blkid);
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

fn make_genesis_block(params: &Params) -> L2Block {
    // TODO maybe fill in with things since the genesis height?
    let l1_seg = L1Segment::new(Vec::new(), Vec::new());

    // TODO this is a total stub, we have to fill it in with something
    let exec_seg = ExecSegment::new(Vec::new());

    let body = L2BlockBody::new(l1_seg, exec_seg);

    // Assemble the genesis header template, pulling in data from whatever
    // sources we need.
    // FIXME this isn't the right timestamp to start the blockchain, this should
    // definitely be pulled from the database or the rollup parameters maybe
    let genesis_ts = params.rollup().l1_start_block_height;
    let zero_blkid = L2BlockId::from(Buf32::zero());
    let genesis_sr = Buf32::zero();
    let tmplt =
        block_template::create_header_template(0, genesis_ts, zero_blkid, &body, genesis_sr);

    let gheader = tmplt.complete_with(Buf64::zero());

    L2Block::new(gheader, body)
}

fn make_genesis_client_state(
    gblock: &L2Block,
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
