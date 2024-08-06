use std::sync::Arc;

use express_reth_db::WitnessProvider;
use express_rpc_utils::to_jsonrpsee_error;
use jsonrpsee::core::RpcResult;
use reth_primitives::B256;

use crate::{AlpenRpcApiServer, BlockWitness};

/// rpc implementation
#[derive(Debug, Clone)]
pub struct AlpenRPC<DB: Clone + Sized> {
    db: Arc<DB>,
}

impl<DB: Clone + Sized> AlpenRPC<DB> {
    /// Create new instance
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

impl<DB> AlpenRpcApiServer for AlpenRPC<DB>
where
    DB: WitnessProvider + Send + Sync + Clone + 'static,
{
    #[doc = "fetch block execution witness data for proving in zkvm"]
    fn block_witness(
        &self,
        block_hash: B256,
        json: Option<bool>,
    ) -> RpcResult<Option<BlockWitness>> {
        let res = if json.unwrap_or(false) {
            self.db
                .get_block_witness(block_hash)
                .map(|maybe_witness| maybe_witness.map(BlockWitness::Json))
        } else {
            self.db
                .get_block_witness_raw(block_hash)
                .map(|maybe_witness| maybe_witness.map(BlockWitness::Raw))
        };

        res.map_err(to_jsonrpsee_error("Failed fetching witness"))
    }
}
