//! Bridge Msg operation interface

use std::sync::Arc;

use alpen_express_primitives::relay::types::BridgeMessage;
use alpen_express_db::traits::*;

use crate::exec::*;

/// Database context for an database operation interface.
pub struct Context<D: Database> {
    db: Arc<D>,
}

impl<D: Database + Sync + Send + 'static> Context<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }

    pub fn into_ops(self, pool: threadpool::ThreadPool) -> BridgeMsgOps {
        BridgeMsgOps::new(pool, Arc::new(self))
    }
}

//
inst_ops! {
    (BridgeMsgOps, Context<D: Database>) {
         write_msg(id: u128, msg: BridgeMessage) => ();
         delete_msgs_before_timestamp(msg_ids: u128) => ();
         get_msgs_by_scope(scope: Vec<u8>) => Vec<BridgeMessage>;
    }
}

fn write_msg<D: Database>(context: &Context<D>, id: u128, msg: BridgeMessage) -> DbResult<()> {
    let chs_store = context.db.bridge_msg_store();
    chs_store.write_msg(id, msg)
}

fn delete_msgs_before_timestamp<D: Database>(context: &Context<D>, msg_ids: u128) -> DbResult<()> {
    let chs_store = context.db.bridge_msg_store();

    chs_store.delete_msgs_before_timestamp(msg_ids)
}

fn get_msgs_by_scope<D: Database>(
    context: &Context<D>,
    scope: Vec<u8>,
) -> DbResult<Vec<BridgeMessage>> {
    let chs_store = context.db.bridge_msg_store();

    chs_store.get_msgs_by_scope(&scope)
}
