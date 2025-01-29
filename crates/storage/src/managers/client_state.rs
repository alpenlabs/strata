//! Client state manager.
// TODO should this also include sync events?

use std::sync::Arc;

use strata_db::traits::Database;
use strata_state::operation::ClientUpdateOutput;
use threadpool::ThreadPool;

use crate::{cache, ops};

pub struct ClientStateManager {
    ops: ops::client_state::ClientStateOps,
    state_cache: cache::CacheTable<u64, Option<ClientUpdateOutput>>,
}

impl ClientStateManager {
    pub fn new<D: Database + Sync + Send + 'static>(pool: ThreadPool, db: Arc<D>) -> Self {
        let ops = ops::client_state::Context::new(db.client_state_db().clone()).into_ops(pool);
        let state_cache = cache::CacheTable::new(64.try_into().unwrap());
        Self { ops, state_cache }
    }
}
