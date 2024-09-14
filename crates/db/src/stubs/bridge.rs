use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use alpen_express_primitives::buf::Buf32;

use crate::{
    entities::bridge_tx_state::BridgeTxState,
    traits::{BridgeTxDatabase, BridgeTxProvider, BridgeTxStore},
    DbResult,
};

#[derive(Debug, Clone, Default)]
pub struct StubTxStateStorage {
    pub db: Arc<StubTxStateDb>,
}

#[derive(Debug, Default)]
pub struct StubTxStateDb(RwLock<HashMap<Buf32, BridgeTxState>>);

impl BridgeTxStore for StubTxStateDb {
    fn put_tx_state(&self, txid: Buf32, tx_state: BridgeTxState) -> DbResult<()> {
        let mut db = self.0.write().unwrap();
        db.insert(txid, tx_state);

        Ok(())
    }

    fn evict_tx_state(&self, txid: Buf32) -> DbResult<Option<BridgeTxState>> {
        let mut db = self.0.write().unwrap();

        Ok(db.remove(&txid))
    }
}

impl BridgeTxProvider for StubTxStateDb {
    fn get_tx_state(&self, txid: Buf32) -> DbResult<Option<BridgeTxState>> {
        let db = self.0.read().unwrap();
        let tx_state = db.get(&txid).cloned();

        Ok(tx_state)
    }
}

impl BridgeTxDatabase for StubTxStateStorage {
    type Store = StubTxStateDb;
    type Provider = StubTxStateDb;

    fn bridge_tx_provider(&self) -> &Arc<Self::Provider> {
        &self.db
    }

    fn bridge_tx_store(&self) -> &Arc<Self::Store> {
        &self.db
    }
}
