use std::sync::Arc;

use super::traits::*;

/// Shim database type that assumes that all the database impls are wrapped in
/// `Arc`s and that the provider and stores are actually the same types.  We
/// might actually use this in practice, it's just for testing.
pub struct CommonDatabase<L1, L2, S, Cl, Ch>
where
    L1: L1DataStore + L1DataProvider + Sync + Send + 'static,
    L2: L2DataStore + L2DataProvider + Sync + Send + 'static,
    S: SyncEventStore + SyncEventProvider + Sync + Send + 'static,
    Cl: ClientStateStore + ClientStateProvider + Sync + Send + 'static,
    Ch: ChainstateStore + ChainstateProvider + Sync + Send + 'static,
{
    l1db: Arc<L1>,
    l2db: Arc<L2>,
    sedb: Arc<S>,
    csdb: Arc<Cl>,
    chdb: Arc<Ch>,
}

impl<L1, L2, S, Cl, Ch> CommonDatabase<L1, L2, S, Cl, Ch>
where
    L1: L1DataStore + L1DataProvider + Sync + Send + 'static,
    L2: L2DataStore + L2DataProvider + Sync + Send + 'static,
    S: SyncEventStore + SyncEventProvider + Sync + Send + 'static,
    Cl: ClientStateStore + ClientStateProvider + Sync + Send + 'static,
    Ch: ChainstateStore + ChainstateProvider + Sync + Send + 'static,
{
    pub fn new(l1db: Arc<L1>, l2db: Arc<L2>, sedb: Arc<S>, csdb: Arc<Cl>, chdb: Arc<Ch>) -> Self {
        Self {
            l1db,
            l2db,
            sedb,
            csdb,
            chdb,
        }
    }
}

impl<L1, L2, S, Cl, Ch> Database for CommonDatabase<L1, L2, S, Cl, Ch>
where
    L1: L1DataStore + L1DataProvider + Sync + Send + 'static,
    L2: L2DataStore + L2DataProvider + Sync + Send + 'static,
    S: SyncEventStore + SyncEventProvider + Sync + Send + 'static,
    Cl: ClientStateStore + ClientStateProvider + Sync + Send + 'static,
    Ch: ChainstateStore + ChainstateProvider + Sync + Send + 'static,
{
    type L1Store = L1;
    type L1Prov = L1;
    type L2Store = L2;
    type L2Prov = L2;
    type SeStore = S;
    type SeProv = S;
    type CsStore = Cl;
    type CsProv = Cl;
    type ChsStore = Ch;
    type ChsProv = Ch;

    fn l1_store(&self) -> &Arc<Self::L1Store> {
        &self.l1db
    }

    fn l1_provider(&self) -> &Arc<Self::L1Prov> {
        &self.l1db
    }

    fn l2_store(&self) -> &Arc<Self::L2Store> {
        &self.l2db
    }

    fn l2_provider(&self) -> &Arc<Self::L2Prov> {
        &self.l2db
    }

    fn sync_event_store(&self) -> &Arc<Self::SeStore> {
        &self.sedb
    }

    fn sync_event_provider(&self) -> &Arc<Self::SeProv> {
        &self.sedb
    }

    fn client_state_store(&self) -> &Arc<Self::CsStore> {
        &self.csdb
    }

    fn client_state_provider(&self) -> &Arc<Self::CsProv> {
        &self.csdb
    }

    fn chainstate_store(&self) -> &Arc<Self::ChsStore> {
        &self.chdb
    }

    fn chainstate_provider(&self) -> &Arc<Self::ChsProv> {
        &self.chdb
    }
}
