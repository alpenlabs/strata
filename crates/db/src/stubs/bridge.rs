use std::{collections::HashMap, sync::RwLock};

use alpen_express_primitives::buf::Buf32;

use crate::{entities::bridge_tx_state::BridgeTxState, traits::BridgeTxDatabase, DbResult};

#[derive(Debug, Default)]
pub struct StubTxStateDb(RwLock<HashMap<Buf32, BridgeTxState>>);

impl BridgeTxDatabase for StubTxStateDb {
    fn put_tx_state(&self, txid: Buf32, tx_state: BridgeTxState) -> DbResult<()> {
        let mut db = self.0.write().unwrap();
        db.insert(txid, tx_state);

        Ok(())
    }

    fn delete_tx_state(&self, txid: Buf32) -> DbResult<Option<BridgeTxState>> {
        let mut db = self.0.write().unwrap();

        Ok(db.remove(&txid))
    }

    fn get_tx_state(&self, txid: Buf32) -> DbResult<Option<BridgeTxState>> {
        let db = self.0.read().unwrap();
        let tx_state = db.get(&txid).cloned();

        Ok(tx_state)
    }
}
