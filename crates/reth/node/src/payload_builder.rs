use std::sync::Arc;

use alloy_consensus::{BlockHeader, Header, Transaction};
use alloy_eips::Typed2718;
use reth::{
    builder::{components::PayloadServiceBuilder, BuilderContext},
    providers::StateProviderFactory,
    revm::database::StateProviderDatabase,
};
use reth_basic_payload_builder::*;
use reth_chainspec::{ChainSpec, ChainSpecProvider, EthChainSpec, EthereumHardforks};
use reth_errors::{BlockExecutionError, BlockValidationError};
use reth_ethereum_payload_builder::EthereumBuilderConfig;
use reth_ethereum_primitives::TransactionSigned;
use reth_evm::{
    env::EvmEnv,
    execute::{BlockBuilder, BlockBuilderOutcome},
    Evm, NextBlockEnvAttributes,
};
use reth_node_api::{ConfigureEvm, FullNodeTypes, NodeTypes, PayloadBuilderAttributes, TxTy};
use reth_payload_builder::{EthBuiltPayload, PayloadBuilderError, PayloadBuilderHandle};
use reth_primitives::{EthPrimitives, InvalidTransactionError};
use reth_primitives_traits::SignedTransaction;
use reth_transaction_pool::{
    error::{Eip4844PoolTransactionError, InvalidPoolTransactionError},
    BestTransactions, BestTransactionsAttributes, PoolTransaction, TransactionPool,
    ValidPoolTransaction,
};
use revm::{context::Block, database::State};
use revm_primitives::U256;
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
    ) -> Result<EvmEnv, <StrataEvmConfig as ConfigureEvm>::Error> {
        let next_attributes = NextBlockEnvAttributes {
            timestamp: attributes.timestamp(),
            suggested_fee_recipient: attributes.suggested_fee_recipient(),
            prev_randao: attributes.prev_randao(),
            gas_limit: parent.gas_limit,
            parent_beacon_block_root: parent.parent_beacon_block_root(),
            withdrawals: Some(attributes.withdrawals().clone()),
        };

        self.evm_config.next_evm_env(parent, &next_attributes)
    }

    /// `StrataPayloadBuilder` constructor.
    pub fn new(
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
        let _evm_env = self
            .evm_env(&args.config.attributes, &args.config.parent_header)
            .map_err(PayloadBuilderError::other)?;

        let _res = default_ethereum_payload(
            self.evm_config.clone(),
            self.client.clone(),
            self.pool.clone(),
            self.builder_config.clone(),
            args,
            |attributes| self.pool.best_transactions_with_attributes(attributes),
        );

        todo!("Implement try_build");
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
        Types: NodeTypes<
            Payload = StrataEngineTypes,
            ChainSpec = ChainSpec,
            Primitives = StrataPrimitives,
        >,
    >,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TxTy<Node::Types>>>
        + Unpin
        + 'static,
{
    async fn spawn_payload_builder_service(
        self,
        _ctx: &BuilderContext<Node>,
        _pool: Pool,
    ) -> eyre::Result<PayloadBuilderHandle<<Node::Types as NodeTypes>::Payload>> {
        tracing::info!("Spawning a custom payload builder");

        // Ok(StrataPayloadBuilder::new(
        //     ctx.provider().clone(),
        //     pool,
        //     ctx.chain_spec().clone(),
        //     EthereumBuilderConfig::new(ctx.payload_builder_config().extra_data_bytes()),
        // ))
        todo!("Implement spawn_payload_builder_service");
    }
}

type BestTransactionsIter<Pool> = Box<
    dyn BestTransactions<Item = Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>,
>;

/// Constructs an Ethereum transaction payload using the best transactions from the pool.
///
/// Given build arguments including an Ethereum client, transaction pool,
/// and configuration, this function creates a transaction payload. Returns
/// a result indicating success with the payload or an error in case of failure.
#[inline]
pub fn default_ethereum_payload<EvmConfig, Client, Pool, F>(
    evm_config: EvmConfig,
    client: Client,
    pool: Pool,
    builder_config: EthereumBuilderConfig,
    args: BuildArguments<StrataPayloadBuilderAttributes, StrataBuiltPayload>,
    best_txs: F,
) -> Result<BuildOutcome<EthBuiltPayload>, PayloadBuilderError>
where
    EvmConfig: ConfigureEvm<Primitives = EthPrimitives, NextBlockEnvCtx = NextBlockEnvAttributes>,
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec = ChainSpec>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>,
    F: FnOnce(BestTransactionsAttributes) -> BestTransactionsIter<Pool>,
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

    let state_provider = client.state_by_block_hash(parent_header.hash())?;
    let state = StateProviderDatabase::new(&state_provider);
    let mut db = State::builder()
        .with_database(cached_reads.as_db_mut(state))
        .with_bundle_update()
        .build();

    let mut builder = evm_config
        .builder_for_next_block(
            &mut db,
            &parent_header,
            NextBlockEnvAttributes {
                timestamp: attributes.timestamp(),
                suggested_fee_recipient: attributes.suggested_fee_recipient(),
                prev_randao: attributes.prev_randao(),
                gas_limit: builder_config.gas_limit(parent_header.gas_limit),
                parent_beacon_block_root: attributes.parent_beacon_block_root(),
                withdrawals: Some(attributes.withdrawals().clone()),
            },
        )
        .map_err(PayloadBuilderError::other)?;

    let chain_spec = client.chain_spec();

    debug!(target: "payload_builder", id=%attributes.inner.id, parent_header = ?parent_header.hash(), parent_number = parent_header.number, "building new payload");
    let mut cumulative_gas_used = 0;
    let block_gas_limit: u64 = builder.evm_mut().block().gas_limit;
    let base_fee = builder.evm_mut().block().basefee;

    let mut best_txs = best_txs(BestTransactionsAttributes::new(
        base_fee,
        builder
            .evm_mut()
            .block()
            .blob_gasprice()
            .map(|gasprice| gasprice as u64),
    ));
    let mut total_fees = U256::ZERO;

    builder.apply_pre_execution_changes().map_err(|err| {
        warn!(target: "payload_builder", %err, "failed to apply pre-execution changes");
        PayloadBuilderError::Internal(err.into())
    })?;

    let mut block_blob_count = 0;
    let blob_params = chain_spec.blob_params_at_timestamp(attributes.inner.timestamp);
    let max_blob_count = blob_params
        .as_ref()
        .map(|params| params.max_blob_count)
        .unwrap_or_default();

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

        let gas_used = match builder.execute_transaction(tx.clone()) {
            Ok(gas_used) => gas_used,
            Err(BlockExecutionError::Validation(BlockValidationError::InvalidTx {
                error, ..
            })) => {
                if error.is_nonce_too_low() {
                    // if the nonce is too low, we can skip this transaction
                    trace!(target: "payload_builder", %error, ?tx, "skipping nonce too low transaction");
                } else {
                    // if the transaction is invalid, we can skip it and all of its
                    // descendants
                    trace!(target: "payload_builder", %error, ?tx, "skipping invalid transaction and its descendants");
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
            Err(err) => return Err(PayloadBuilderError::evm(err)),
        };

        // add to the total blob gas used if the transaction successfully executed
        if let Some(blob_tx) = tx.as_eip4844() {
            block_blob_count += blob_tx.blob_versioned_hashes.len() as u64;

            // if we've reached the max blob count, we can skip blob txs entirely
            if block_blob_count == max_blob_count {
                best_txs.skip_blobs();
            }
        }

        // update add to total fees
        let miner_fee = tx
            .effective_tip_per_gas(base_fee)
            .expect("fee is always valid; execution succeeded");
        total_fees += U256::from(miner_fee) * U256::from(gas_used);
        cumulative_gas_used += gas_used;
    }

    // check if we have a better block
    if !is_better_payload(best_payload.as_ref(), total_fees) {
        // Release db
        drop(builder);
        // can skip building the block
        return Ok(BuildOutcome::Aborted {
            fees: total_fees,
            cached_reads,
        });
    }

    let BlockBuilderOutcome {
        execution_result,
        block,
        ..
    } = builder.finish(&state_provider)?;

    let requests = chain_spec
        .is_prague_active_at_timestamp(attributes.timestamp())
        .then_some(execution_result.requests);

    // initialize empty blob sidecars at first. If cancun is active then this will
    let mut blob_sidecars = Vec::new();

    // only determine cancun fields when active
    if chain_spec.is_cancun_active_at_timestamp(attributes.timestamp()) {
        // grab the blob sidecars from the executed txs
        blob_sidecars = pool
            .get_all_blobs_exact(
                block
                    .body()
                    .transactions()
                    .filter(|tx| tx.is_eip4844())
                    .map(|tx| *tx.tx_hash())
                    .collect(),
            )
            .map_err(PayloadBuilderError::other)?;
    }

    let sealed_block = Arc::new(block.sealed_block().clone());
    debug!(target: "payload_builder", id=%attributes.inner.id, sealed_block_header = ?sealed_block.sealed_header(), "sealed built block");

    let mut payload = EthBuiltPayload::new(attributes.inner.id, sealed_block, total_fees, requests);

    // extend the payload with the blob sidecars from the executed txs
    payload.extend_sidecars(blob_sidecars.into_iter().map(Arc::unwrap_or_clone));

    Ok(BuildOutcome::Better {
        payload,
        cached_reads,
    })
}
