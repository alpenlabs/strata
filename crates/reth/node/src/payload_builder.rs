use std::sync::Arc;

use alloy_consensus::{Header, Transaction, EMPTY_OMMER_ROOT_HASH};
use alloy_eips::{
    eip4844::DATA_GAS_PER_BLOB, eip6110, eip7685::Requests, merge::BEACON_NONCE, Typed2718,
};
use alpen_reth_evm::collect_withdrawal_intents;
use reth::{
    builder::{components::PayloadServiceBuilder, BuilderContext, PayloadBuilderConfig},
    providers::{ExecutionOutcome, StateProviderFactory},
    revm::database::StateProviderDatabase,
};
use reth_basic_payload_builder::*;
use reth_chainspec::{ChainSpec, ChainSpecProvider, EthChainSpec, EthereumHardforks};
use reth_errors::RethError;
use reth_ethereum_payload_builder::EthereumBuilderConfig;
use reth_ethereum_primitives::{Block, BlockBody, Receipt, TransactionSigned};
use reth_evm::{
    env::EvmEnv, system_calls::SystemCaller, ConfigureEvmEnv, Evm, EvmError, InvalidTxError,
    NextBlockEnvAttributes,
};
use reth_evm_ethereum::eip6110::parse_deposits_from_receipts;
use reth_node_api::{
    ConfigureEvm, FullNodeTypes, NodeTypesWithEngine, PayloadBuilderAttributes, TxTy,
};
use reth_payload_builder::{EthBuiltPayload, PayloadBuilderError};
use reth_primitives::InvalidTransactionError;
use reth_primitives_traits::{
    proofs::{self},
    Block as _,
};
use reth_transaction_pool::{
    error::{Eip4844PoolTransactionError, InvalidPoolTransactionError},
    BestTransactions, BestTransactionsAttributes, PoolTransaction, TransactionPool,
};
use revm::{
    db::{states::bundle_state::BundleRetention, State},
    DatabaseCommit,
};
use revm_primitives::{calc_excess_blob_gas, ResultAndState, U256};
use tracing::{debug, trace, warn};

use crate::{
    engine::StrataEngineTypes,
    evm::StrataEvmConfig,
    node::StrataPrimitives,
    payload::{StrataBuiltPayload, StrataPayloadBuilderAttributes},
};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StrataPayloadBuilder<Pool, Client> {
    /// Client providing access to node state.
    client: Client,
    /// Transaction pool.
    pool: Pool,
    /// The type responsible for creating the evm.
    evm_config: StrataEvmConfig,
    /// Payload builder configuration.
    builder_config: EthereumBuilderConfig,
}

impl<Pool, Client> StrataPayloadBuilder<Pool, Client> {
    /// Returns the configured [`EvmEnv`] for the targeted payload
    /// (that has the `parent` as its parent).
    pub fn evm_env(
        &self,
        attributes: &StrataPayloadBuilderAttributes,
        parent: &Header,
    ) -> Result<EvmEnv, <StrataEvmConfig as ConfigureEvmEnv>::Error> {
        let next_attributes = NextBlockEnvAttributes {
            timestamp: attributes.timestamp(),
            suggested_fee_recipient: attributes.suggested_fee_recipient(),
            prev_randao: attributes.prev_randao(),
            gas_limit: parent.gas_limit,
        };
        self.evm_config.next_evm_env(parent, next_attributes)
    }

    /// `StrataPayloadBuilder` constructor.
    pub const fn new(
        client: Client,
        pool: Pool,
        chain_spec: Arc<ChainSpec>,
        builder_config: EthereumBuilderConfig,
    ) -> Self {
        Self {
            client,
            pool,
            evm_config: StrataEvmConfig::new(chain_spec),
            builder_config,
        }
    }
}

impl<Pool, Client> PayloadBuilder for StrataPayloadBuilder<Pool, Client>
where
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec = ChainSpec> + Clone,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>,
{
    type Attributes = StrataPayloadBuilderAttributes;
    type BuiltPayload = StrataBuiltPayload;

    fn try_build(
        &self,
        args: BuildArguments<Self::Attributes, Self::BuiltPayload>,
    ) -> Result<BuildOutcome<Self::BuiltPayload>, PayloadBuilderError> {
        let evm_env = self
            .evm_env(&args.config.attributes, &args.config.parent_header)
            .map_err(PayloadBuilderError::other)?;

        try_build_payload(
            self.evm_config.clone(),
            self.client.clone(),
            self.pool.clone(),
            self.builder_config.clone(),
            args,
            evm_env,
        )
    }

    fn build_empty_payload(
        &self,
        config: PayloadConfig<Self::Attributes>,
    ) -> Result<Self::BuiltPayload, PayloadBuilderError> {
        let PayloadConfig {
            parent_header,
            attributes,
        } = config;

        // use default eth payload builder
        let eth_build_payload = <reth_ethereum_payload_builder::EthereumPayloadBuilder<
            Pool,
            Client,
        > as PayloadBuilder>::build_empty_payload(
            &reth_ethereum_payload_builder::EthereumPayloadBuilder::new(
                self.client.clone(),
                self.pool.clone(),
                self.evm_config.inner().clone(),
                self.builder_config.clone(),
            ),
            PayloadConfig {
                parent_header,
                attributes: attributes.inner,
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
        Types: NodeTypesWithEngine<
            Engine = StrataEngineTypes,
            ChainSpec = ChainSpec,
            Primitives = StrataPrimitives,
        >,
    >,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TxTy<Node::Types>>>
        + Unpin
        + 'static,
{
    type PayloadBuilder = StrataPayloadBuilder<Pool, Node::Provider>;

    async fn build_payload_builder(
        &self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<Self::PayloadBuilder> {
        Ok(StrataPayloadBuilder::new(
            ctx.provider().clone(),
            pool,
            ctx.chain_spec().clone(),
            EthereumBuilderConfig::new(ctx.payload_builder_config().extra_data_bytes()),
        ))
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
    client: Client,
    pool: Pool,
    builder_config: EthereumBuilderConfig,
    args: BuildArguments<StrataPayloadBuilderAttributes, StrataBuiltPayload>,
    evm_env: EvmEnv<EvmConfig::Spec>,
) -> Result<BuildOutcome<StrataBuiltPayload>, PayloadBuilderError>
where
    EvmConfig: ConfigureEvm<Header = Header, Transaction = TransactionSigned>,
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec = ChainSpec>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>,
{
    let BuildArguments {
        mut cached_reads,
        config,
        cancel,
        best_payload,
    } = args;

    let PayloadConfig {
        parent_header,
        attributes,
    } = config;

    // convert to eth payload
    let best_payload = best_payload.map(|p| p.inner);

    let chain_spec = client.chain_spec();
    let state_provider = client.state_by_block_hash(parent_header.hash())?;
    let state = StateProviderDatabase::new(state_provider);
    let mut db = State::builder()
        .with_database(cached_reads.as_db_mut(state))
        .with_bundle_update()
        .build();

    debug!(target: "payload_builder", id=%attributes.payload_id(), parent_hash = ?parent_header.hash(), parent_number = parent_header.number, "building new payload");

    let mut cumulative_gas_used = 0;
    let block_gas_limit: u64 = evm_env.block_env.gas_limit.to::<u64>();
    let base_fee = evm_env.block_env.basefee.to::<u64>();

    let mut executed_senders = Vec::new();
    let mut executed_txs = Vec::new();

    let mut best_txs = pool.best_transactions_with_attributes(BestTransactionsAttributes::new(
        base_fee,
        evm_env
            .block_env
            .get_blob_gasprice()
            .map(|gasprice| gasprice as u64),
    ));

    let mut total_fees = U256::ZERO;

    let block_number = evm_env.block_env.number.to::<u64>();
    let beneficiary = evm_env.block_env.coinbase;

    let mut sys_calls = SystemCaller::new(evm_config.clone(), chain_spec.clone());

    // apply eip-4788 pre block contract call
    sys_calls
        .pre_block_beacon_root_contract_call(
            &mut db,
            &evm_env,
            attributes.parent_beacon_block_root(),
        )
        .map_err(|err| {
            warn!(target: "payload_builder",
                parent_hash=%parent_header.hash(),
                %err,
                "failed to apply beacon root contract call for empty payload"
            );
            PayloadBuilderError::Internal(err.into())
        })?;

    // apply eip-2935 blockhashes update
    sys_calls
        .pre_block_blockhashes_contract_call(&mut db, &evm_env, parent_header.hash())
        .map_err(|err| PayloadBuilderError::Internal(err.into()))?;

    let mut evm = evm_config.evm_with_env(&mut db, evm_env);

    let mut receipts = Vec::new();
    let mut block_blob_count = 0;
    let blob_params = chain_spec.blob_params_at_timestamp(attributes.inner.timestamp);
    let max_blob_count = blob_params
        .as_ref()
        .map(|params| params.max_blob_count)
        .unwrap_or_default();
    // let mut withdrawal_intents = Vec::new();

    while let Some(pool_tx) = best_txs.next() {
        // ensure we still have capacity for this transaction
        if cumulative_gas_used + pool_tx.gas_limit() > block_gas_limit {
            // we can't fit this transaction into the block, so we need to mark it as invalid
            // which also removes all dependent transaction from the iterator before we can
            // continue
            best_txs.mark_invalid(
                &pool_tx,
                InvalidPoolTransactionError::ExceedsGasLimit(pool_tx.gas_limit(), block_gas_limit),
            );
            continue;
        }

        // check if the job was cancelled, if so we can exit early
        if cancel.is_cancelled() {
            return Ok(BuildOutcome::Cancelled);
        }

        // convert tx to a signed transaction
        let tx = pool_tx.to_consensus();

        // There's only limited amount of blob space available per block, so we need to check if
        // the EIP-4844 can still fit in the block
        if let Some(blob_tx) = tx.as_eip4844() {
            let tx_blob_count = blob_tx.blob_versioned_hashes.len() as u64;
            if block_blob_count + tx_blob_count > max_blob_count {
                // we can't fit this _blob_ transaction into the block, so we mark it as
                // invalid, which removes its dependent transactions from
                // the iterator. This is similar to the gas limit condition
                // for regular transactions above.
                trace!(target: "payload_builder", tx=?tx.hash(), ?block_blob_count, "skipping blob transaction because it would exceed the max blob count per block");
                best_txs.mark_invalid(
                    &pool_tx,
                    InvalidPoolTransactionError::Eip4844(
                        Eip4844PoolTransactionError::TooManyEip4844Blobs {
                            have: block_blob_count + tx_blob_count,
                            permitted: max_blob_count,
                        },
                    ),
                );
                continue;
            }
        }

        // Configure the environment for the tx.
        let tx_env = evm_config.tx_env(tx.tx(), tx.signer());

        let ResultAndState { result, state } = match evm.transact(tx_env) {
            Ok(res) => res,
            Err(err) => {
                if let Some(err) = err.as_invalid_tx_err() {
                    if err.is_nonce_too_low() {
                        // if the nonce is too low, we can skip this transaction
                        trace!(target: "payload_builder", %err, ?tx, "skipping nonce too low transaction");
                    } else {
                        // if the transaction is invalid, we can skip it and all of its
                        // descendants
                        trace!(target: "payload_builder", %err, ?tx, "skipping invalid transaction and its descendants");
                        best_txs.mark_invalid(
                            &pool_tx,
                            InvalidPoolTransactionError::Consensus(
                                InvalidTransactionError::TxTypeNotSupported,
                            ),
                        );
                    }
                    continue;
                }

                // this is an error that we should treat as fatal for this attempt
                return Err(PayloadBuilderError::evm(err));
            }
        };
        debug!(?result, "EVM transaction executed");
        // commit changes
        evm.db_mut().commit(state);

        // add to the total blob gas used if the transaction successfully executed
        if let Some(blob_tx) = tx.as_eip4844() {
            block_blob_count += blob_tx.blob_versioned_hashes.len() as u64;

            // if we've reached the max blob count, we can skip blob txs entirely
            if block_blob_count == max_blob_count {
                best_txs.skip_blobs();
            }
        }

        let gas_used = result.gas_used();

        // add gas used by the transaction to cumulative gas used, before creating the receipt
        cumulative_gas_used += gas_used;

        // Push transaction changeset and calculate header bloom filter for receipt.
        #[allow(clippy::needless_update)] // side-effect of optimism fields
        receipts.push(Receipt {
            tx_type: tx.tx_type(),
            success: result.is_success(),
            cumulative_gas_used,
            logs: result.into_logs().into_iter().collect(),
            ..Default::default()
        });

        // update add to total fees
        let miner_fee = tx
            .effective_tip_per_gas(base_fee)
            .expect("fee is always valid; execution succeeded");
        total_fees += U256::from(miner_fee) * U256::from(gas_used);

        // append sender and transaction to the respective lists
        executed_senders.push(tx.signer());
        executed_txs.push(tx.into_tx());
    }

    // check if we have a better block
    if !is_better_payload(best_payload.as_ref(), total_fees) {
        // drop evm so db is released.
        drop(evm);
        // can skip building the block
        return Ok(BuildOutcome::Aborted {
            fees: total_fees,
            cached_reads,
        });
    }

    // calculate the requests and the requests root
    let requests = if chain_spec.is_prague_active_at_timestamp(attributes.timestamp()) {
        let deposit_requests = parse_deposits_from_receipts(&chain_spec, receipts.iter())
            .map_err(|err| PayloadBuilderError::Internal(RethError::Execution(err.into())))?;

        let mut requests = Requests::default();

        if !deposit_requests.is_empty() {
            requests.push_request_with_type(eip6110::DEPOSIT_REQUEST_TYPE, deposit_requests);
        }

        requests.extend(
            sys_calls
                .apply_post_execution_changes(&mut evm)
                .map_err(|err| PayloadBuilderError::Internal(err.into()))?,
        );

        Some(requests)
    } else {
        None
    };

    // Release db
    drop(evm);

    // NOTE: bridge-ins are currently handled through withdrawals
    let withdrawals_root = commit_withdrawals(
        &mut db,
        &chain_spec,
        attributes.timestamp(),
        attributes.withdrawals(),
    )?;

    let withdrawal_intents: Vec<_> =
        collect_withdrawal_intents(executed_txs.iter().zip(receipts.iter())).collect();

    // merge all transitions into bundle state, this would apply the withdrawal balance changes
    // and 4788 contract call
    db.merge_transitions(BundleRetention::Reverts);
    let requests_hash = requests.as_ref().map(|requests| requests.requests_hash());

    let execution_outcome = ExecutionOutcome::new(
        db.take_bundle(),
        vec![receipts],
        block_number,
        vec![requests.clone().unwrap_or_default()],
    );
    let receipts_root = execution_outcome
        .ethereum_receipts_root(block_number)
        .expect("Number is in range");
    let logs_bloom = execution_outcome
        .block_logs_bloom(block_number)
        .expect("Number is in range");

    // calculate the state root
    let hashed_state = db.database.db.hashed_post_state(execution_outcome.state());
    let (state_root, _) = {
        db.database
            .inner()
            .state_root_with_updates(hashed_state.clone())
            .inspect_err(|err| {
                warn!(target: "payload_builder",
                    parent_hash=%parent_header.hash(),
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
        blob_sidecars = pool
            .get_all_blobs_exact(
                executed_txs
                    .iter()
                    .filter(|tx| tx.is_eip4844())
                    .map(|tx| *tx.hash())
                    .collect(),
            )
            .map_err(PayloadBuilderError::other)?;

        excess_blob_gas = if chain_spec.is_cancun_active_at_timestamp(attributes.timestamp()) {
            let parent_excess_blob_gas = parent_header.excess_blob_gas.unwrap_or_default();
            let parent_blob_gas_used = parent_header.blob_gas_used.unwrap_or_default();
            let parent_target_blob_gas_per_block =
                parent_header.excess_blob_gas.unwrap_or_default();
            Some(calc_excess_blob_gas(
                parent_excess_blob_gas,
                parent_blob_gas_used,
                parent_target_blob_gas_per_block,
            ))
        } else {
            // for the first post-fork block, both parent.blob_gas_used and
            // parent.excess_blob_gas are evaluated as 0
            Some(calc_excess_blob_gas(0, 0, 0))
        };

        blob_gas_used = Some(block_blob_count * DATA_GAS_PER_BLOB);
    }

    let header = Header {
        parent_hash: parent_header.hash(),
        ommers_hash: EMPTY_OMMER_ROOT_HASH,
        beneficiary,
        state_root,
        transactions_root,
        receipts_root,
        withdrawals_root,
        logs_bloom,
        timestamp: attributes.timestamp(),
        mix_hash: attributes.prev_randao(),
        nonce: BEACON_NONCE.into(),
        base_fee_per_gas: Some(base_fee),
        number: parent_header.number + 1,
        gas_limit: block_gas_limit,
        difficulty: U256::ZERO,
        gas_used: cumulative_gas_used,
        extra_data: builder_config.extra_data,
        parent_beacon_block_root: attributes.parent_beacon_block_root(),
        blob_gas_used,
        excess_blob_gas,
        requests_hash,
    };

    let withdrawals = chain_spec
        .is_shanghai_active_at_timestamp(attributes.timestamp())
        .then(|| attributes.withdrawals().clone());

    // seal the block
    let block = Block {
        header,
        body: BlockBody {
            transactions: executed_txs,
            ommers: vec![],
            withdrawals,
        },
    };

    let sealed_block = Arc::new(block.seal_slow());
    debug!(target: "payload_builder", ?sealed_block, "sealed built block");

    let mut eth_payload =
        EthBuiltPayload::new(attributes.inner.id, sealed_block, total_fees, requests);

    // extend the payload with the blob sidecars from the executed txs
    eth_payload.extend_sidecars(blob_sidecars.into_iter().map(Arc::unwrap_or_clone));

    let payload = StrataBuiltPayload::new(eth_payload, withdrawal_intents);

    Ok(BuildOutcome::Better {
        payload,
        cached_reads,
    })
}
