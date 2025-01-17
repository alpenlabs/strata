use std::{collections::HashMap, sync::Arc};

use alloy_rpc_types::EIP1186AccountProofResponse;
use eyre::eyre;
use futures_util::TryStreamExt;
use reth_evm::execute::{BlockExecutionInput, BlockExecutorProvider, Executor};
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::FullNodeComponents;
use reth_primitives::{
    revm_primitives::alloy_primitives::{Address, B256},
    BlockNumHash, BlockWithSenders, TransactionSigned,
};
use reth_provider::{BlockReader, Chain, ExecutionOutcome, StateProviderFactory};
use reth_revm::{db::CacheDB, primitives::FixedBytes};
use reth_rpc_types_compat::proof::from_primitive_account_proof;
use reth_trie::{HashedPostState, TrieInput};
use strata_proofimpl_evm_ee_stf::{mpt::proofs_to_tries, EvmBlockStfInput};
use strata_reth_db::WitnessStore;
use tracing::{debug, error};

use crate::{
    alloy2reth::IntoReth,
    cache_db_provider::{AccessedState, CacheDBProvider},
};

pub struct ProverWitnessGenerator<Node: FullNodeComponents, S: WitnessStore + Clone> {
    ctx: ExExContext<Node>,
    db: Arc<S>,
}

impl<Node: FullNodeComponents, S: WitnessStore + Clone> ProverWitnessGenerator<Node, S> {
    pub fn new(ctx: ExExContext<Node>, db: Arc<S>) -> Self {
        Self { ctx, db }
    }

    fn commit(&mut self, chain: &Chain) -> eyre::Result<Option<BlockNumHash>> {
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

            let prover_input = extract_zkvm_input(block_hash, &self.ctx, &outcome)?;

            // TODO: maybe put db writes in another thread
            if let Err(err) = self.db.put_block_witness(block_hash, &prover_input) {
                error!(?err, ?block_hash);
                break;
            }

            finished_height = Some(BlockNumHash::new(outcome.first_block(), block_hash))
        }

        Ok(finished_height)
    }

    pub async fn start(mut self) -> eyre::Result<()> {
        debug!("start prover witness generator");
        while let Some(notification) = self.ctx.notifications.try_next().await? {
            if let Some(committed_chain) = notification.committed_chain() {
                let finished_height = self.commit(&committed_chain)?;
                if let Some(finished_height) = finished_height {
                    self.ctx
                        .events
                        .send(ExExEvent::FinishedHeight(finished_height))?;
                }
            }
        }

        Ok(())
    }
}

fn get_accessed_states<'a, Node: FullNodeComponents>(
    ctx: &ExExContext<Node>,
    block: &'a BlockWithSenders,
    block_idx: u64,
) -> eyre::Result<AccessedState> {
    let executor: <Node as FullNodeComponents>::Executor = ctx.block_executor().clone();
    let provider = ctx.provider().history_by_block_number(block_idx)?;

    let cache_db_provider = CacheDBProvider::new(provider);
    let cache_db = CacheDB::new(&cache_db_provider);

    let block_exec_input: BlockExecutionInput<'a, BlockWithSenders> =
        BlockExecutionInput::new(block, block.difficulty);

    executor.executor(cache_db).execute(block_exec_input)?;

    let acessed_state = cache_db_provider.get_accessed_state();
    Ok(acessed_state)
}

fn extract_zkvm_input<Node: FullNodeComponents>(
    block_id: FixedBytes<32>,
    ctx: &ExExContext<Node>,
    exec_outcome: &ExecutionOutcome,
) -> eyre::Result<EvmBlockStfInput> {
    let current_block = ctx
        .provider()
        .block_by_hash(block_id)?
        .ok_or(eyre!("Failed to get current block"))?;
    let current_block_idx = current_block.number;

    let withdrawals = current_block
        .body
        .clone()
        .withdrawals
        .unwrap_or_default()
        .into_iter()
        .map(|el| el.into_reth())
        .collect();

    let prev_block_idx = current_block_idx - 1;
    let previous_provider = ctx.provider().history_by_block_number(prev_block_idx)?;
    let prev_block = ctx
        .provider()
        .block_by_number(prev_block_idx)?
        .ok_or(eyre!("Failed to get prev block"))?;

    // Call the magic function here:
    let block_execution_input = current_block
        .clone()
        .with_recovered_senders()
        .ok_or(eyre!("failed to recover senders"))?;

    let accessed_states = get_accessed_states(ctx, &block_execution_input, prev_block_idx)?;

    let current_block_txns = current_block
        .body
        .transactions()
        .cloned()
        .collect::<Vec<TransactionSigned>>();

    let prev_state_root = prev_block.state_root;

    let mut parent_proofs: HashMap<Address, EIP1186AccountProofResponse> = HashMap::new();
    let mut current_proofs: HashMap<Address, EIP1186AccountProofResponse> = HashMap::new();
    let contracts = accessed_states.accessed_contracts;

    // Accumulate account proof of account in previous block
    for (accessed_address, accessed_slots) in accessed_states.accessed_accounts.iter() {
        let slots: Vec<B256> = accessed_slots
            .iter()
            .map(|el| B256::from_slice(el.as_le_slice()))
            .collect();

        // Apply empty bundle state over previous block state.
        let proof = previous_provider.proof(
            TrieInput::from_state(HashedPostState::from_bundle_state([])),
            *accessed_address,
            &slots,
        )?;
        let proof = from_primitive_account_proof(proof);

        parent_proofs.insert(*accessed_address, proof);
    }

    // Accumulate account proof of account in current block
    for (accessed_address, accessed_slots) in accessed_states.accessed_accounts.iter() {
        let slots: Vec<B256> = accessed_slots
            .iter()
            .map(|el| B256::from_slice(el.as_le_slice()))
            .collect();

        let proof = previous_provider.proof(
            TrieInput::from_state(exec_outcome.hash_state_slow()),
            *accessed_address,
            &slots,
        )?;
        let proof = from_primitive_account_proof(proof);

        current_proofs.insert(*accessed_address, proof);
    }

    let (state_trie, storage) = proofs_to_tries(
        prev_state_root,
        parent_proofs.clone(),
        current_proofs.clone(),
    )
    .expect("Proof to tries infallible");

    let input = EvmBlockStfInput {
        beneficiary: current_block.header.beneficiary,
        gas_limit: current_block.gas_limit,
        timestamp: current_block.header.timestamp,
        extra_data: current_block.header.extra_data,
        mix_hash: current_block.header.mix_hash,
        transactions: current_block_txns,
        withdrawals,
        pre_state_trie: state_trie,
        pre_state_storage: storage,
        contracts,
        parent_header: prev_block.header,
        // NOTE: using default to save prover cost.
        // Will need to revisit if BLOCKHASH opcode operation is a blocker
        ancestor_headers: Default::default(),
    };

    Ok(input)
}
