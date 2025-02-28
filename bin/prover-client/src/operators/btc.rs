use std::sync::Arc;

use strata_btcio::rpc::{traits::ReaderRpc, BitcoinClient};
use strata_l1tx::filter::TxFilterConfig;
use strata_primitives::{
    l1::L1BlockCommitment,
    params::RollupParams,
    proof::{ProofContext, ProofKey},
};
use strata_proofimpl_btc_blockspace::{logic::BlockScanProofInput, program::BtcBlockspaceProgram};
use strata_rocksdb::prover::db::ProofDb;
use tracing::error;

use super::ProvingOp;
use crate::errors::ProvingTaskError;

/// A struct that implements the [`ProvingOp`] trait for Bitcoin blockspace proof generation.
///
/// It interfaces with the Bitcoin blockchain via a [`BitcoinClient`] to fetch the necessary data
/// required by the [`BtcBlockspaceProgram`] for the proof generation.
#[derive(Debug, Clone)]
pub struct BtcBlockspaceOperator {
    pub btc_client: Arc<BitcoinClient>,
    rollup_params: Arc<RollupParams>,
}

impl BtcBlockspaceOperator {
    /// Creates a new BTC operations instance.
    pub fn new(btc_client: Arc<BitcoinClient>, rollup_params: Arc<RollupParams>) -> Self {
        Self {
            btc_client,
            rollup_params,
        }
    }
}

impl ProvingOp for BtcBlockspaceOperator {
    type Program = BtcBlockspaceProgram;

    type Params = (L1BlockCommitment, L1BlockCommitment);

    fn construct_proof_ctx(
        &self,
        btc_range: &Self::Params,
    ) -> Result<ProofContext, ProvingTaskError> {
        let (start, end) = btc_range;
        // Do some sanity checks
        assert!(
            end.height() >= start.height(),
            "failed to construct Btc blockspace proof context. start_height: {} > end_height {}",
            start.height(),
            end.height()
        );
        Ok(ProofContext::BtcBlockspace(*start, *end))
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        _db: &ProofDb,
    ) -> Result<BlockScanProofInput, ProvingTaskError> {
        let (start, end) = match task_id.context() {
            ProofContext::BtcBlockspace(start, end) => (*start, *end),
            _ => return Err(ProvingTaskError::InvalidInput("BtcBlockspace".to_string())),
        };

        let mut btc_blocks = vec![];
        let mut current_block_id = *end.blkid();
        loop {
            let btc_block = self
                .btc_client
                .get_block(&current_block_id.into())
                .await
                .inspect_err(|_| error!(%current_block_id, "Failed to fetch BTC BlockId"))
                .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

            let prev_block_hash = btc_block.header.prev_blockhash;

            btc_blocks.push(btc_block);

            if start.blkid() == &current_block_id {
                break;
            } else {
                current_block_id = prev_block_hash.into();
            }
        }

        // Reverse the blocks to make them in ascending order
        btc_blocks.reverse();

        let tx_filters =
            TxFilterConfig::derive_from(&self.rollup_params).expect("failed to derive tx filters");
        Ok(BlockScanProofInput {
            btc_blocks,
            tx_filters,
        })
    }
}
