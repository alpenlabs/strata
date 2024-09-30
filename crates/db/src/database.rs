use std::sync::Arc;

use super::traits::*;

/// Shim database type that assumes that all the database impls are wrapped in
/// `Arc`s and that the provider and stores are actually the same types.  We
/// might actually use this in practice, it's just for testing.
pub struct CommonDatabase<L1Db, L2Db, SyncEventDb, ClientStateDb, ChainStateDb, CheckpointDb>
where
    L1Db: L1DataStore + L1DataProvider + Sync + Send + 'static,
    L2Db: L2DataStore + L2DataProvider + Sync + Send + 'static,
    SyncEventDb: SyncEventStore + SyncEventProvider + Sync + Send + 'static,
    ClientStateDb: ClientStateStore + ClientStateProvider + Sync + Send + 'static,
    ChainStateDb: ChainstateStore + ChainstateProvider + Sync + Send + 'static,
    CheckpointDb: CheckpointStore + CheckpointProvider + Sync + Send + 'static,
{
    l1_db: Arc<L1Db>,
    l2_db: Arc<L2Db>,
    sync_event_db: Arc<SyncEventDb>,
    client_state_db: Arc<ClientStateDb>,
    chain_state_db: Arc<ChainStateDb>,
    checkpoint_db: Arc<CheckpointDb>,
}

impl<L1Db, L2Db, SyncEventDb, ClientStateDb, ChainStateDb, CheckpointDb>
    CommonDatabase<L1Db, L2Db, SyncEventDb, ClientStateDb, ChainStateDb, CheckpointDb>
where
    L1Db: L1DataStore + L1DataProvider + Sync + Send + 'static,
    L2Db: L2DataStore + L2DataProvider + Sync + Send + 'static,
    SyncEventDb: SyncEventStore + SyncEventProvider + Sync + Send + 'static,
    ClientStateDb: ClientStateStore + ClientStateProvider + Sync + Send + 'static,
    ChainStateDb: ChainstateStore + ChainstateProvider + Sync + Send + 'static,
    CheckpointDb: CheckpointStore + CheckpointProvider + Sync + Send + 'static,
{
    pub fn new(
        l1_db: Arc<L1Db>,
        l2_db: Arc<L2Db>,
        sync_event_db: Arc<SyncEventDb>,
        client_state_db: Arc<ClientStateDb>,
        chain_state_db: Arc<ChainStateDb>,
        checkpoint_db: Arc<CheckpointDb>,
    ) -> Self {
        Self {
            l1_db,
            l2_db,
            sync_event_db,
            client_state_db,
            chain_state_db,
            checkpoint_db,
        }
    }
}

impl<L1Db, L2Db, SyncEventDb, ClientStateDb, ChainStateDb, CheckpointDb> Database
    for CommonDatabase<L1Db, L2Db, SyncEventDb, ClientStateDb, ChainStateDb, CheckpointDb>
where
    L1Db: L1DataStore + L1DataProvider + Sync + Send + 'static,
    L2Db: L2DataStore + L2DataProvider + Sync + Send + 'static,
    SyncEventDb: SyncEventStore + SyncEventProvider + Sync + Send + 'static,
    ClientStateDb: ClientStateStore + ClientStateProvider + Sync + Send + 'static,
    ChainStateDb: ChainstateStore + ChainstateProvider + Sync + Send + 'static,
    CheckpointDb: CheckpointStore + CheckpointProvider + Sync + Send + 'static,
{
    type L1DataStore = L1Db;
    type L1Provider = L1Db;
    type L2DataStore = L2Db;
    type L2DataProv = L2Db;
    type SyncEventStore = SyncEventDb;
    type SyncEventProvider = SyncEventDb;
    type ClientStateStore = ClientStateDb;
    type ClientStateProvider = ClientStateDb;
    type ChainStateStore = ChainStateDb;
    type ChainStateProvider = ChainStateDb;
    type CheckpointProvider = CheckpointDb;
    type CheckpointStore = CheckpointDb;

    fn l1_store(&self) -> &Arc<Self::L1DataStore> {
        &self.l1_db
    }

    fn l1_provider(&self) -> &Arc<Self::L1Provider> {
        &self.l1_db
    }

    fn l2_store(&self) -> &Arc<Self::L2DataStore> {
        &self.l2_db
    }

    fn l2_provider(&self) -> &Arc<Self::L2DataProv> {
        &self.l2_db
    }

    fn sync_event_store(&self) -> &Arc<Self::SyncEventStore> {
        &self.sync_event_db
    }

    fn sync_event_provider(&self) -> &Arc<Self::SyncEventProvider> {
        &self.sync_event_db
    }

    fn client_state_store(&self) -> &Arc<Self::ClientStateStore> {
        &self.client_state_db
    }

    fn client_state_provider(&self) -> &Arc<Self::ClientStateProvider> {
        &self.client_state_db
    }

    fn chain_state_store(&self) -> &Arc<Self::ChainStateStore> {
        &self.chain_state_db
    }

    fn chain_state_provider(&self) -> &Arc<Self::ChainStateProvider> {
        &self.chain_state_db
    }

    fn checkpoint_store(&self) -> &Arc<Self::CheckpointStore> {
        &self.checkpoint_db
    }

    fn checkpoint_provider(&self) -> &Arc<Self::CheckpointProvider> {
        &self.checkpoint_db
    }
}
