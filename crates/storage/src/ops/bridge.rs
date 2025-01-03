use std::sync::Arc;

use bitcoin::Txid;
use strata_db::{entities::bridge_tx_state::BridgeTxState, traits::BridgeTxDatabase, DbResult};
use strata_primitives::buf::Buf32;

use crate::exec::*;

inst_ops_simple! {
    (<D: BridgeTxDatabase> => BridgeTxStateOps) {
        get_tx_state(txid: Buf32) => Option<BridgeTxState>;
        put_tx_state(txid: Buf32, tx_state: BridgeTxState) => ();
        delete_tx_state(txid: Buf32) => Option<BridgeTxState>;
    }
}
