//! Bridge Msg operation interface

use std::sync::Arc;

use alpen_express_bridge_msg::types::BridgeMessage;
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
         write_msg(id: u64, msg: BridgeMessage) => ();
         delete_msgs_before_timestamp(msg_ids: u64) => ();
         get_msgs_by_scope(scope: Vec<u8>) => Option<BridgeMessage>;
    }
}

fn write_msg<D: Database>(context: &Context<D>, id: u64, msg: BridgeMessage) -> DbResult<()> {
    let chs_store = context.db.bridge_msg_store();
    chs_store.write_msg(id, msg)
}

fn delete_msgs_before_timestamp<D: Database>(context: &Context<D>, msg_ids: u64) -> DbResult<()> {
    let chs_store = context.db.bridge_msg_store();

    chs_store.delete_msgs_before_timestamp(msg_ids)
}

fn get_msgs_by_scope<D: Database>(
    context: &Context<D>,
    scope: Vec<u8>,
) -> DbResult<Option<BridgeMessage>> {
    let chs_store = context.db.bridge_msg_store();

    chs_store.get_msgs_by_scope(&scope)
}
