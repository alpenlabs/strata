use bitcoin::Block;
use strata_l1tx::filter::{indexer::index_block, TxFilterConfig};
use strata_primitives::{
    epoch::EpochCommitment,
    l1::{L1BlockManifest, L1BlockTxOps, ProtocolOperation},
    params::Params,
};
use strata_state::batch::SignedCheckpoint;
use strata_storage::ChainstateManager;

use crate::tx_indexer::ReaderTxVisitorImpl;

pub trait TxFilterconfigProvider {
    fn get_filterconfig_at_epoch(&self, epoch: &EpochCommitment) -> TxFilterConfig;
}

pub struct DbTxFilterconfigProvider<'a> {
    chainstatedb_manager: &'a ChainstateManager,
    params: &'a Params,
}

impl<'a> DbTxFilterconfigProvider<'a> {
    pub fn new(chainstatedb_manager: &'a ChainstateManager, params: &'a Params) -> Self {
        Self {
            chainstatedb_manager,
            params,
        }
    }
}

impl TxFilterconfigProvider for DbTxFilterconfigProvider<'_> {
    fn get_filterconfig_at_epoch(&self, epoch: &EpochCommitment) -> TxFilterConfig {
        if epoch.is_null() {
            // no checkpoint seen yet, so just get filter config based on rollup params
            // TODO: use genesis chainstate instead ?
            TxFilterConfig::derive_from(self.params.rollup()).expect("derive filterconfig")
        } else {
            // have seen a checkpoint. Next blocks must be parsed based on this state.
            let final_block_commitment = epoch.to_block_commitment();
            let chainstate = self
                .chainstatedb_manager
                .get_toplevel_chainstate_blocking(final_block_commitment.slot())
                .expect("TODO: retry on db errors")
                .expect("missing chainstate");
            TxFilterConfig::derive_from_chainstate(self.params.rollup(), &chainstate)
                .expect("derive filterconfig")
        }
    }
}

fn parse_protocolops(block: &Block, config: &TxFilterConfig) -> Vec<ProtocolOperation> {
    let txns = index_block(block, ReaderTxVisitorImpl::new, config);
    txns.into_iter()
        .flat_map(|entry| entry.into_contents().into_parts().0)
        .collect()
}

fn first_checkpoint<'a, I>(protocol_ops: I) -> Option<&'a SignedCheckpoint>
where
    I: IntoIterator<Item = &'a ProtocolOperation>,
{
    protocol_ops.into_iter().find_map(|op| match op {
        ProtocolOperation::Checkpoint(signed_checkpoint) => Some(signed_checkpoint),
        _ => None,
    })
}

pub fn parse_l1blocktxops(
    manifests: &[L1BlockManifest],
    checkpoint_epoch: &EpochCommitment,
    filterconfig_provider: &impl TxFilterconfigProvider,
) -> Vec<L1BlockTxOps> {
    let mut epoch = checkpoint_epoch.epoch();
    // derive tx filter config based on last seen checkpoint
    let mut filter_config = filterconfig_provider.get_filterconfig_at_epoch(checkpoint_epoch);

    let mut l1blocks = Vec::new();
    for manifest in manifests {
        let protocol_ops = parse_protocolops(&manifest.get_block(), &filter_config);

        // If we see a valid checkpoint, all subsequent blocks must use new filterconfig derived
        // from this. We are guaranteed to have the chainstate corresponding to this checkpoint.
        let checkpoint = first_checkpoint(&protocol_ops);
        if let Some(checkpoint) = checkpoint {
            filter_config = filterconfig_provider.get_filterconfig_at_epoch(
                &checkpoint.checkpoint().batch_info().get_epoch_commitment(),
            );
            epoch = checkpoint
                .checkpoint()
                .batch_info()
                .get_epoch_commitment()
                .epoch();
        }

        // FIXME: deposit utxos should be added to filterconfig as soon as they are seen, even in
        // the middle of an epoch so as to not miss any transactions that spend them.

        let block = L1BlockTxOps::new(epoch, manifest.record().clone(), protocol_ops);
        l1blocks.push(block);
    }

    l1blocks
}
