use std::{collections::HashMap, sync::Arc};

use alloy_rpc_types::EIP1186AccountProofResponse;
use express_reth_db::WitnessStore;
use eyre::eyre;
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::FullNodeComponents;
use reth_primitives::{Address, TransactionSignedNoHash};
use reth_provider::{BlockReader, Chain, StateProviderFactory};
use reth_revm::db::BundleState;
use reth_rpc_types_compat::proof::from_primitive_account_proof;
use tracing::{debug, error};
use zkvm_primitives::{mpt::proofs_to_tries, ZKVMInput};

pub struct ProverWitnessGenerator<Node: FullNodeComponents, S: WitnessStore + Clone> {
    ctx: ExExContext<Node>,
    db: Arc<S>,
}

impl<Node: FullNodeComponents, S: WitnessStore + Clone> ProverWitnessGenerator<Node, S> {
    pub fn new(ctx: ExExContext<Node>, db: Arc<S>) -> Self {
        Self { ctx, db }
    }

    fn commit(&mut self, chain: &Chain) -> eyre::Result<Option<u64>> {
        let mut finished_height = None;
        let blocks = chain.blocks();
        let bundles = chain.range().filter_map(|block_number| {
            blocks
                .get(&block_number)
                .map(|block| block.hash())
                .zip(chain.execution_outcome_at_block(block_number))
        });

        for (block_hash, outcome) in bundles {
            #[cfg(debug_assertions)]
            assert!(outcome.len() == 1, "should only contain single block");

            // TODO: extract correct prover_input from execution outcome
            // let prover_input = extract_zkvm_input(outcome)?;
            let prover_input = ZKVMInput::default();

            // TODO: maybe put db writes in another thread ?
            if let Err(err) = self.db.put_block_witness(block_hash, &prover_input) {
                error!(?err, ?block_hash);

                break;
            }

            finished_height = Some(outcome.first_block())
        }

        Ok(finished_height)
    }

    pub async fn start(mut self) -> eyre::Result<()> {
        debug!("start prover witness generator");
        while let Some(notification) = self.ctx.notifications.recv().await {
            if let Some(committed_chain) = notification.committed_chain() {
                let finished_height = self.commit(&committed_chain)?;
                if let Some(finished_height) = finished_height {
                    self.ctx
                        .events
                        .send(ExExEvent::FinishedHeight(finished_height))?;
                }
            }
            // remove witnesses on chain reverts ?
        }

        Ok(())
    }
}

// FIXME
fn extract_zkvm_input<Node: FullNodeComponents>(
    ctx: &ExExContext<Node>,
    new: Arc<Chain>,
) -> eyre::Result<ZKVMInput> {
    let (current_block_num, _) = new
        .blocks()
        .first_key_value()
        .ok_or(eyre!("Failed to get current block"))?;
    let previous_provider = ctx
        .provider()
        .history_by_block_number(current_block_num - 1)?;
    let current_provider = ctx.provider().latest()?;

    let current_block = ctx
        .provider()
        .block_by_number(current_block_num.clone())?
        .ok_or(eyre!("Failed to get current block"))?;

    let current_block_txns = current_block
        .body
        .clone()
        .into_iter()
        .map(|tx| TransactionSignedNoHash::from(tx))
        .collect::<Vec<TransactionSignedNoHash>>();

    let prev_block = ctx
        .provider()
        .block_by_number(current_block_num - 1)?
        .ok_or(eyre!("Failed to get prev block"))?;
    let prev_state_root = prev_block.state_root;

    let previous_bundle_state = BundleState::default();
    let current_bundle_state = &new.execution_outcome().bundle;

    let mut parent_proofs: HashMap<Address, EIP1186AccountProofResponse> = HashMap::new();
    let mut current_proofs: HashMap<Address, EIP1186AccountProofResponse> = HashMap::new();

    // Accumulate account proof of account in previous block
    for address in current_bundle_state.state().keys() {
        let proof = previous_provider.proof(&previous_bundle_state, *address, &[])?;

        let proof = from_primitive_account_proof(proof);
        parent_proofs.insert(*address, proof);
    }

    // Accumulate account proof of account in current block
    for address in current_bundle_state.state().keys() {
        let proof = current_provider.proof(current_bundle_state, *address, &[])?;
        let proof = from_primitive_account_proof(proof);
        current_proofs.insert(*address, proof);
    }

    let (state_trie, storage) = proofs_to_tries(
        prev_state_root,
        parent_proofs.clone(),
        current_proofs.clone(),
    )
    .expect("Proof to tries infallable");

    let input = ZKVMInput {
        beneficiary: current_block.header.beneficiary,
        gas_limit: current_block.gas_limit,
        timestamp: current_block.header.timestamp,
        extra_data: current_block.header.extra_data,
        mix_hash: current_block.header.mix_hash,
        transactions: current_block_txns,
        withdrawals: Vec::new(),
        parent_state_trie: state_trie,
        parent_storage: storage,
        contracts: Default::default(),
        parent_header: prev_block.header,
        ancestor_headers: Default::default(),
    };

    Ok(input)
}
