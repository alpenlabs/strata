//! High level sync manager which controls core sync tasks and manages sync
//! status.  Exposes handles to interact with fork choice manager and CSM
//! executor and other core sync pipeline tasks.

use std::sync::Arc;
use std::thread;

use tokio::sync::{broadcast, mpsc};
use tracing::*;

use alpen_vertex_db::traits::{Database, L2DataProvider};
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_primitives::params::Params;

use crate::ctl::CsmController;
use crate::message::{ClientUpdateNotif, CsmMessage, ForkChoiceMessage};
use crate::unfinalized_tracker::UnfinalizedBlockTracker;
use crate::{errors, fork_choice_manager, genesis, unfinalized_tracker, worker};

pub struct SyncManager {
    params: Arc<Params>,

    chain_tip_msg_tx: mpsc::Sender<ForkChoiceMessage>,
    csm_ctl: Arc<CsmController>,
    cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
}

impl SyncManager {
    pub fn params(&self) -> &Params {
        &self.params
    }

    pub fn get_params(&self) -> Arc<Params> {
        self.params.clone()
    }

    /// Gets a ref to the CSM controller.
    pub fn csm_controller(&self) -> &CsmController {
        &self.csm_ctl
    }

    /// Gets a clone of the CSM controller.
    pub fn get_csm_ctl(&self) -> Arc<CsmController> {
        self.csm_ctl.clone()
    }

    /// Returns a new broadcast `Receiver` handle to the consensus update
    /// notification queue.  Provides no guarantees about which position in the
    /// queue will be returned on the first receive.
    pub fn create_cstate_subscription(&self) -> broadcast::Receiver<Arc<ClientUpdateNotif>> {
        self.cupdate_rx.resubscribe()
    }

    /// Submits a fork choice message if possible. (synchronously)
    pub fn submit_chain_tip_msg(&self, ctm: ForkChoiceMessage) -> bool {
        self.chain_tip_msg_tx.blocking_send(ctm).is_ok()
    }

    /// Submits a fork choice message if possible. (asynchronously)
    pub async fn submit_chain_tip_msg_async(&self, ctm: ForkChoiceMessage) -> bool {
        self.chain_tip_msg_tx.send(ctm).await.is_ok()
    }
}

/// Starts the sync tasks using provided settings.
pub fn start_sync_tasks<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
>(
    database: Arc<D>,
    engine: Arc<E>,
    pool: Arc<threadpool::ThreadPool>,
    params: Arc<Params>,
) -> anyhow::Result<SyncManager> {
    // Create channels.
    let (ctm_tx, ctm_rx) = mpsc::channel::<ForkChoiceMessage>(64);
    let (csm_tx, csm_rx) = mpsc::channel::<CsmMessage>(64);
    let csm_ctl = Arc::new(CsmController::new(database.clone(), pool, csm_tx));

    // TODO should this be in an `Arc`?  it's already fairly compact so we might
    // not be benefitting from the reduced cloning
    let (cupdate_tx, cupdate_rx) = broadcast::channel::<Arc<ClientUpdateNotif>>(64);

    // Check if we have to do genesis.
    if genesis::check_needs_genesis(database.as_ref())? {
        info!("we need to do genesis!");
        genesis::init_genesis_states(&params, database.as_ref())?;
    }

    // Init the consensus worker state and get the current state from it.
    let cw_state = worker::WorkerState::open(params.clone(), database.clone(), cupdate_tx)?;
    let cur_state = cw_state.cur_state().clone();
    let cur_tip_blkid = *cur_state.chain_tip_blkid();
    let fin_tip_blkid = *cur_state.finalized_blkid();

    // Get the block's index.
    let l2_prov = database.l2_provider();
    let tip_block = l2_prov
        .get_block_data(cur_tip_blkid)?
        .ok_or(errors::Error::MissingL2Block(cur_tip_blkid))?;
    let cur_tip_index = tip_block.header().blockidx();

    let fin_block = l2_prov
        .get_block_data(fin_tip_blkid)?
        .ok_or(errors::Error::MissingL2Block(fin_tip_blkid))?;
    let fin_tip_index = fin_block.header().blockidx();

    // Init the chain tracker from the state we figured out.
    let mut chain_tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(fin_tip_blkid);
    load_unfinalized_blocks(fin_tip_index + 1, database.clone(), &mut chain_tracker)?;
    let ct_state = fork_choice_manager::ForkChoiceManager::new(
        params.clone(),
        database.clone(),
        cur_state,
        chain_tracker,
        cur_tip_blkid,
        cur_tip_index,
    );

    // Start core threads.
    // TODO set up watchdog for these things
    let eng_ct = engine.clone();
    let eng_cw = engine.clone();
    let ctl_ct = csm_ctl.clone();
    let ct_handle =
        thread::spawn(|| fork_choice_manager::tracker_task(ct_state, eng_ct, ctm_rx, ctl_ct));
    let cw_handle = thread::spawn(|| worker::consensus_worker_task(cw_state, eng_cw, csm_rx));

    // TODO do something with the handles

    Ok(SyncManager {
        params,
        chain_tip_msg_tx: ctm_tx,
        csm_ctl,
        cupdate_rx,
    })
}

// TODO: sending only the l2_provider seems sufficient instead of passing the entire database
// This can be moved to [`UnfinalizedBlockTracker`]
pub fn load_unfinalized_blocks<D>(
    height: u64,
    database: Arc<D>,
    chain_tracker: &mut UnfinalizedBlockTracker,
) -> anyhow::Result<()>
where
    D: Database,
{
    let mut height = height;
    let l2_prov = database.l2_provider();
    while let Ok(block_ids) = l2_prov.get_blocks_at_height(height) {
        if block_ids.is_empty() {
            break;
        }
        for block_id in block_ids {
            if let Some(block) = l2_prov.get_block_data(block_id)? {
                let header = block.header();
                let _ = chain_tracker.attach_block(block_id, header);
            }
        }
        height += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::ArbitraryGenerator;
    use alpen_vertex_db::traits::L2DataStore;
    use alpen_vertex_state::{
        block::{L2Block, L2BlockBody, L2BlockHeader},
        id::L2BlockId,
    };

    use super::*;

    fn get_genesis_block() -> L2Block {
        let arb = ArbitraryGenerator::new();
        let mut header: L2BlockHeader = arb.generate();
        let empty_hash = L2BlockId::default();
        let body: L2BlockBody = arb.generate();
        header.set_parent_and_idx(empty_hash, 0);
        L2Block::new(header, body)
    }

    fn get_mock_block_with_parent(parent: &L2BlockHeader) -> L2Block {
        let arb = ArbitraryGenerator::new();
        let mut header: L2BlockHeader = arb.generate();
        let body: L2BlockBody = arb.generate();
        header.set_parent_and_idx(parent.get_blockid(), parent.blockidx() + 1);
        L2Block::new(header, body)
    }

    fn setup_test_chain(
        l2_prov: &impl L2DataStore,
    ) -> (L2BlockHeader, L2BlockHeader, L2BlockHeader, L2BlockHeader) {
        let genesis = get_genesis_block();
        let genesis_header = genesis.header().clone();

        let block1 = get_mock_block_with_parent(genesis.header());
        let block1_header = block1.header().clone();

        let block2 = get_mock_block_with_parent(block1.header());
        let block2_header = block2.header().clone();

        let block2a = get_mock_block_with_parent(block1.header());
        let block2a_header = block2a.header().clone();

        l2_prov.put_block_data(genesis.clone()).unwrap();
        l2_prov.put_block_data(block1.clone()).unwrap();
        l2_prov.put_block_data(block2.clone()).unwrap();
        l2_prov.put_block_data(block2a.clone()).unwrap();

        (genesis_header, block1_header, block2_header, block2a_header)
    }

    #[test]
    fn test_load_unfinalized_blocks() {
        // b2   b2a (side chain)
        // |   /
        // | /
        // b1 (finalized)
        // |
        // g1 (10)
        // |

        let db = alpen_test_utils::get_common_db();
        let l2_prov = db.l2_store();

        let (genesis, block1, block2, block2a) = setup_test_chain(l2_prov.as_ref());

        // Init the chain tracker from the state we figured out.
        let mut chain_tracker =
            unfinalized_tracker::UnfinalizedBlockTracker::new_empty(genesis.get_blockid());

        load_unfinalized_blocks(1, db, &mut chain_tracker).unwrap();

        assert_eq!(
            chain_tracker.get_parent(&block1.get_blockid()),
            Some(&genesis.get_blockid())
        );

        assert_eq!(
            chain_tracker.get_parent(&block2.get_blockid()),
            Some(&block1.get_blockid())
        );

        assert_eq!(
            chain_tracker.get_parent(&block2a.get_blockid()),
            Some(&block1.get_blockid())
        );
    }
}
