use std::sync::Arc;

use reth::{
    builder::{components::PayloadServiceBuilder, BuilderContext, PayloadBuilderConfig},
    providers::{CanonStateSubscriptions, ExecutionOutcome, StateProviderFactory},
    revm::database::StateProviderDatabase,
    transaction_pool::{BestTransactionsAttributes, TransactionPool},
};
use reth_basic_payload_builder::{
    commit_withdrawals, is_better_payload, BasicPayloadJobGenerator,
    BasicPayloadJobGeneratorConfig, BuildArguments, BuildOutcome, PayloadBuilder, PayloadConfig,
    WithdrawalsOutcome,
};
use reth_chain_state::ExecutedBlock;
use reth_chainspec::{ChainSpec, ChainSpecProvider, EthereumHardforks};
use reth_errors::RethError;
use reth_evm::system_calls::SystemCaller;
use reth_evm_ethereum::{eip6110::parse_deposits_from_receipts, EthEvmConfig};
use reth_node_api::{ConfigureEvm, FullNodeTypes, NodeTypesWithEngine, PayloadBuilderAttributes};
use reth_payload_builder::{
    EthBuiltPayload, PayloadBuilderError, PayloadBuilderHandle, PayloadBuilderService,
};
use reth_primitives::{
    constants::{eip4844::MAX_DATA_GAS_PER_BLOCK, BEACON_NONCE},
    proofs::{self, calculate_requests_root},
    Block, BlockBody, Header, Receipt, Requests, EMPTY_OMMER_ROOT_HASH,
};
use reth_trie::HashedPostState;
use revm::{
    db::{states::bundle_state::BundleRetention, State},
    DatabaseCommit,
};
use revm_primitives::{
    calc_excess_blob_gas, EVMError, EnvWithHandlerCfg, InvalidTransaction, ResultAndState, U256,
};
use strata_reth_evm::collect_withdrawal_intents;
use tracing::{debug, trace, warn};

use crate::{
    engine::StrataEngineTypes,
    evm::StrataEvmConfig,
    payload::{StrataBuiltPayload, StrataPayloadBuilderAttributes},
};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StrataPayloadBuilder {
    /// The type responsible for creating the evm.
    evm_config: StrataEvmConfig,
}

impl<Pool, Client> PayloadBuilder<Pool, Client> for StrataPayloadBuilder
where
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec = ChainSpec>,
    Pool: TransactionPool,
{
    type Attributes = StrataPayloadBuilderAttributes;
    type BuiltPayload = StrataBuiltPayload;

    fn try_build(
        &self,
        args: BuildArguments<Pool, Client, Self::Attributes, Self::BuiltPayload>,
    ) -> Result<BuildOutcome<Self::BuiltPayload>, PayloadBuilderError> {
        try_build_payload(self.evm_config.clone(), args)
    }

    fn build_empty_payload(
        &self,
        client: &Client,
        config: PayloadConfig<Self::Attributes>,
    ) -> Result<Self::BuiltPayload, PayloadBuilderError> {
        let PayloadConfig {
            parent_block,
            extra_data,
            attributes,
        } = config;

        let chain_spec = client.chain_spec();

        // use default eth payload builder
        let eth_build_payload =
            <reth_ethereum_payload_builder::EthereumPayloadBuilder as PayloadBuilder<
                Pool,
                Client,
            >>::build_empty_payload(
                &reth_ethereum_payload_builder::EthereumPayloadBuilder::new(EthEvmConfig::new(
                    chain_spec.clone(),
                )),
                client,
                PayloadConfig {
                    parent_block,
                    extra_data,
                    attributes: attributes.0,
                },
            )?;
        Ok(StrataBuiltPayload::new(eth_build_payload, Vec::new()))
    }
}

/// A custom payload service builder that supports the custom engine types
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct StrataPayloadServiceBuilder;

impl<Node, Pool> PayloadServiceBuilder<Node, Pool> for StrataPayloadServiceBuilder
where
    Node: FullNodeTypes<
        Types: NodeTypesWithEngine<Engine = StrataEngineTypes, ChainSpec = ChainSpec>,
    >,
    Pool: TransactionPool + Unpin + 'static,
{
    async fn spawn_payload_service(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<PayloadBuilderHandle<<Node::Types as NodeTypesWithEngine>::Engine>> {
        let payload_builder = StrataPayloadBuilder {
            evm_config: StrataEvmConfig::new(ctx.chain_spec()),
        };
        let conf = ctx.payload_builder_config();

        let payload_job_config = BasicPayloadJobGeneratorConfig::default()
            .interval(conf.interval())
            .deadline(conf.deadline())
            .max_payload_tasks(conf.max_payload_tasks())
            .extradata(conf.extradata_bytes());

        let payload_generator = BasicPayloadJobGenerator::with_builder(
            ctx.provider().clone(),
            pool,
            ctx.task_executor().clone(),
            payload_job_config,
            payload_builder,
        );
        let (payload_service, payload_builder) =
            PayloadBuilderService::new(payload_generator, ctx.provider().canonical_state_stream());

        ctx.task_executor()
            .spawn_critical("payload builder service", Box::pin(payload_service));

        Ok(payload_builder)
    }
}

/// Constructs an Ethereum transaction payload using the best transactions from the pool.
///
/// Given build arguments including an Ethereum client, transaction pool,
/// and configuration, this function creates a transaction payload. Returns
/// a res ult indicating success with the payload or an error in case of failure.
///
/// Adapted from
/// [default_ethereum_payload](reth_ethereum_payload_builder::default_ethereum_payload)
#[inline]
pub fn try_build_payload<EvmConfig, Pool, Client>(
    evm_config: EvmConfig,
    args: BuildArguments<Pool, Client, StrataPayloadBuilderAttributes, StrataBuiltPayload>,
) -> Result<BuildOutcome<StrataBuiltPayload>, PayloadBuilderError>
where
    EvmConfig: ConfigureEvm<Header = Header>,
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec = ChainSpec>,
    Pool: TransactionPool,
{
    let BuildArguments {
        client,
        pool,
        mut cached_reads,
        config,
        cancel,
        best_payload,
    } = args;

    // convert to eth payload
    let best_payload = best_payload.map(|p| p.inner);

    let state_provider = client.state_by_block_hash(config.parent_block.hash())?;
    let state = StateProviderDatabase::new(state_provider);
    let mut db = State::builder()
        .with_database_ref(cached_reads.as_db(state))
        .with_bundle_update()
        .build();

    let PayloadConfig {
        parent_block,
        attributes,
        extra_data,
    } = config;
    let chain_spec = client.chain_spec();

    debug!(target: "payload_builder", id=%attributes.payload_id(), parent_hash = ?parent_block.hash(), parent_number = parent_block.number, "building new payload");

    let (initialized_cfg, initialized_block_env) = evm_config.next_cfg_and_block_env(
        parent_block.header(),
        reth_evm::NextBlockEnvAttributes {
            timestamp: attributes.timestamp(),
            suggested_fee_recipient: attributes.suggested_fee_recipient(),
            prev_randao: attributes.prev_randao(),
        },
    );

    let mut cumulative_gas_used = 0;
    let mut sum_blob_gas_used = 0;
    let block_gas_limit: u64 = initialized_block_env
        .gas_limit
        .try_into()
        .unwrap_or(chain_spec.max_gas_limit);
    let base_fee = initialized_block_env.basefee.to::<u64>();

    let mut executed_senders = Vec::new();
    let mut executed_txs = Vec::new();

    let mut best_txs = pool.best_transactions_with_attributes(BestTransactionsAttributes::new(
        base_fee,
        initialized_block_env
            .get_blob_gasprice()
            .map(|gasprice| gasprice as u64),
    ));

    let mut total_fees = U256::ZERO;

    let block_number = initialized_block_env.number.to::<u64>();

    let mut sys_calls = SystemCaller::new(&evm_config, chain_spec.clone());

    // apply eip-4788 pre block contract call
    sys_calls
        .pre_block_beacon_root_contract_call(
            &mut db,
            &initialized_cfg,
            &initialized_block_env,
            attributes.parent_beacon_block_root(),
        )
        .map_err(|err| {
            warn!(target: "payload_builder",
                parent_hash=%parent_block.hash(),
                %err,
                "failed to apply beacon root contract call for empty payload"
            );
            PayloadBuilderError::Internal(err.into())
        })?;

    // apply eip-2935 blockhashes update
    sys_calls
        .pre_block_blockhashes_contract_call(
            &mut db,
            &initialized_cfg,
            &initialized_block_env,
            parent_block.hash(),
        )
        .map_err(|err| PayloadBuilderError::Internal(err.into()))?;

    let mut receipts = Vec::new();
    // let mut withdrawal_intents = Vec::new();
    while let Some(pool_tx) = best_txs.next() {
        // ensure we still have capacity for this transaction
        if cumulative_gas_used + pool_tx.gas_limit() > block_gas_limit {
            // we can't fit this transaction into the block, so we need to mark it as invalid
            // which also removes all dependent transaction from the iterator before we can
            // continue
            best_txs.mark_invalid(&pool_tx);
            continue;
        }

        // check if the job was cancelled, if so we can exit early
        if cancel.is_cancelled() {
            return Ok(BuildOutcome::Cancelled);
        }

        // convert tx to a signed transaction
        let tx = pool_tx.to_recovered_transaction();

        // There's only limited amount of blob space available per block, so we need to check if
        // the EIP-4844 can still fit in the block
        if let Some(blob_tx) = tx.transaction.as_eip4844() {
            let tx_blob_gas = blob_tx.blob_gas();
            if sum_blob_gas_used + tx_blob_gas > MAX_DATA_GAS_PER_BLOCK {
                // we can't fit this _blob_ transaction into the block, so we mark it as
                // invalid, which removes its dependent transactions from
                // the iterator. This is similar to the gas limit condition
                // for regular transactions above.
                trace!(target: "payload_builder", tx=?tx.hash, ?sum_blob_gas_used, ?tx_blob_gas, "skipping blob transaction because it would exceed the max data gas per block");
                best_txs.mark_invalid(&pool_tx);
                continue;
            }
        }

        let env = EnvWithHandlerCfg::new_with_cfg_env(
            initialized_cfg.clone(),
            initialized_block_env.clone(),
            evm_config.tx_env(&tx),
        );

        // Configure the environment for the block.
        let mut evm = evm_config.evm_with_env(&mut db, env);

        let ResultAndState { result, state } = match evm.transact() {
            Ok(res) => res,
            Err(err) => {
                match err {
                    EVMError::Transaction(err) => {
                        if matches!(err, InvalidTransaction::NonceTooLow { .. }) {
                            // if the nonce is too low, we can skip this transaction
                            trace!(target: "payload_builder", %err, ?tx, "skipping nonce too low transaction");
                        } else {
                            // if the transaction is invalid, we can skip it and all of its
                            // descendants
                            trace!(target: "payload_builder", %err, ?tx, "skipping invalid transaction and its descendants");
                            best_txs.mark_invalid(&pool_tx);
                        }

                        continue;
                    }
                    err => {
                        // this is an error that we should treat as fatal for this attempt
                        return Err(PayloadBuilderError::EvmExecutionError(err));
                    }
                }
            }
        };
        debug!(?result, "EVM transaction executed");
        // drop evm so db is released.
        drop(evm);
        // commit changes
        db.commit(state);

        // add to the total blob gas used if the transaction successfully executed
        if let Some(blob_tx) = tx.transaction.as_eip4844() {
            let tx_blob_gas = blob_tx.blob_gas();
            sum_blob_gas_used += tx_blob_gas;

            // if we've reached the max data gas per block, we can skip blob txs entirely
            if sum_blob_gas_used == MAX_DATA_GAS_PER_BLOCK {
                best_txs.skip_blobs();
            }
        }

        let gas_used = result.gas_used();

        // add gas used by the transaction to cumulative gas used, before creating the receipt
        cumulative_gas_used += gas_used;

        // Push transaction changeset and calculate header bloom filter for receipt.
        #[allow(clippy::needless_update)] // side-effect of optimism fields
        receipts.push(Some(Receipt {
            tx_type: tx.tx_type(),
            success: result.is_success(),
            cumulative_gas_used,
            logs: result.into_logs().into_iter().map(Into::into).collect(),
            ..Default::default()
        }));

        // update add to total fees
        let miner_fee = tx
            .effective_tip_per_gas(Some(base_fee))
            .expect("fee is always valid; execution succeeded");
        total_fees += U256::from(miner_fee) * U256::from(gas_used);

        // append sender and transaction to the respective lists
        executed_senders.push(tx.signer());
        executed_txs.push(tx.into_signed());
    }

    // check if we have a better block
    if !is_better_payload(best_payload.as_ref(), total_fees) {
        // can skip building the block
        return Ok(BuildOutcome::Aborted {
            fees: total_fees,
            cached_reads,
        });
    }

    // calculate the requests and the requests root
    let (requests, requests_root) = if chain_spec
        .is_prague_active_at_timestamp(attributes.timestamp())
    {
        let deposit_requests = parse_deposits_from_receipts(&chain_spec, receipts.iter().flatten())
            .map_err(|err| PayloadBuilderError::Internal(RethError::Execution(err.into())))?;
        let withdrawal_requests = sys_calls
            .post_block_withdrawal_requests_contract_call(
                &mut db,
                &initialized_cfg,
                &initialized_block_env,
            )
            .map_err(|err| PayloadBuilderError::Internal(err.into()))?;
        let consolidation_requests = sys_calls
            .post_block_consolidation_requests_contract_call(
                &mut db,
                &initialized_cfg,
                &initialized_block_env,
            )
            .map_err(|err| PayloadBuilderError::Internal(err.into()))?;

        let requests = [
            deposit_requests,
            withdrawal_requests,
            consolidation_requests,
        ]
        .concat();
        let requests_root = calculate_requests_root(&requests);
        (Some(requests.into()), Some(requests_root))
    } else {
        (None, None)
    };

    // NOTE: bridge-ins are currently handled through withdrawals
    let WithdrawalsOutcome {
        withdrawals_root,
        withdrawals,
    } = commit_withdrawals(
        &mut db,
        &chain_spec,
        attributes.timestamp(),
        attributes.withdrawals().clone(),
    )?;

    let withdrawal_intents = collect_withdrawal_intents(receipts.iter().cloned()).collect();

    // merge all transitions into bundle state, this would apply the withdrawal balance changes
    // and 4788 contract call
    db.merge_transitions(BundleRetention::PlainState);

    let execution_outcome = ExecutionOutcome::new(
        db.take_bundle(),
        vec![receipts].into(),
        block_number,
        vec![Requests::default()],
    );
    let receipts_root = execution_outcome
        .receipts_root_slow(block_number)
        .expect("Number is in range");
    let logs_bloom = execution_outcome
        .block_logs_bloom(block_number)
        .expect("Number is in range");

    // calculate the state root
    let hashed_state = HashedPostState::from_bundle_state(&execution_outcome.state().state);
    let (state_root, trie_output) = {
        let state_provider = db.database.0.inner.borrow_mut();
        state_provider
            .db
            .state_root_with_updates(hashed_state.clone())
            .inspect_err(|err| {
                warn!(target: "payload_builder",
                    parent_hash=%parent_block.hash(),
                    %err,
                    "failed to calculate state root for payload"
                );
            })?
    };

    // create the block header
    let transactions_root = proofs::calculate_transaction_root(&executed_txs);

    // initialize empty blob sidecars at first. If cancun is active then this will
    let mut blob_sidecars = Vec::new();
    let mut excess_blob_gas = None;
    let mut blob_gas_used = None;

    // only determine cancun fields when active
    if chain_spec.is_cancun_active_at_timestamp(attributes.timestamp()) {
        // grab the blob sidecars from the executed txs
        blob_sidecars = pool.get_all_blobs_exact(
            executed_txs
                .iter()
                .filter(|tx| tx.is_eip4844())
                .map(|tx| tx.hash)
                .collect(),
        )?;

        excess_blob_gas = if chain_spec.is_cancun_active_at_timestamp(parent_block.timestamp) {
            let parent_excess_blob_gas = parent_block.excess_blob_gas.unwrap_or_default();
            let parent_blob_gas_used = parent_block.blob_gas_used.unwrap_or_default();
            Some(calc_excess_blob_gas(
                parent_excess_blob_gas,
                parent_blob_gas_used,
            ))
        } else {
            // for the first post-fork block, both parent.blob_gas_used and
            // parent.excess_blob_gas are evaluated as 0
            Some(calc_excess_blob_gas(0, 0))
        };

        blob_gas_used = Some(sum_blob_gas_used);
    }

    let header = Header {
        parent_hash: parent_block.hash(),
        ommers_hash: EMPTY_OMMER_ROOT_HASH,
        beneficiary: initialized_block_env.coinbase,
        state_root,
        transactions_root,
        receipts_root,
        withdrawals_root,
        logs_bloom,
        timestamp: attributes.timestamp(),
        mix_hash: attributes.prev_randao(),
        nonce: BEACON_NONCE.into(),
        base_fee_per_gas: Some(base_fee),
        number: parent_block.number + 1,
        gas_limit: block_gas_limit,
        difficulty: U256::ZERO,
        gas_used: cumulative_gas_used,
        extra_data,
        parent_beacon_block_root: attributes.parent_beacon_block_root(),
        blob_gas_used,
        excess_blob_gas,
        requests_root,
    };

    // seal the block
    let block = Block {
        header,
        body: BlockBody {
            transactions: executed_txs,
            ommers: vec![],
            withdrawals,
            requests,
        },
    };

    let sealed_block = block.seal_slow();
    debug!(target: "payload_builder", ?sealed_block, "sealed built block");

    let executed = ExecutedBlock {
        block: Arc::new(sealed_block.clone()),
        senders: Arc::new(executed_senders),
        execution_output: Arc::new(execution_outcome),
        hashed_state: Arc::new(hashed_state),
        trie: Arc::new(trie_output),
    };

    // TODO: None is likely incorrect.
    let mut eth_payload = EthBuiltPayload::new(
        attributes.payload_id(),
        sealed_block,
        total_fees,
        Some(executed),
    );

    // extend the payload with the blob sidecars from the executed txs
    eth_payload.extend_sidecars(blob_sidecars);

    let payload = StrataBuiltPayload::new(eth_payload, withdrawal_intents);

    Ok(BuildOutcome::Better {
        payload,
        cached_reads,
    })
}
