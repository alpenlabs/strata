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
use reth_evm_ethereum::EthEvmConfig;
use reth_node_api::{ConfigureEvm, FullNodeTypes, NodeTypes, PayloadBuilderAttributes, TxTy};
use reth_node_builder::components::PayloadBuilderBuilder;
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
    payload::{StrataBuiltPayload, StrataPayloadBuilderAttributes},
};

/// A custom payload service builder that supports the custom engine types
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct StrataPayloadBuilderBuilder;

impl<Node, Pool> PayloadBuilderBuilder<Node, Pool> for StrataPayloadBuilderBuilder
where
    Node: FullNodeTypes<
        Types: NodeTypes<
            Payload = StrataEngineTypes,
            ChainSpec = ChainSpec,
            Primitives = EthPrimitives,
        >,
    >,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>
        + Unpin
        + 'static,
{
    type PayloadBuilder = StrataPayloadBuilder<Pool, Node::Provider>;

    async fn build_payload_builder(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<Self::PayloadBuilder> {
        let payload_builder = StrataPayloadBuilder {
            inner: reth_ethereum_payload_builder::EthereumPayloadBuilder::new(
                ctx.provider().clone(),
                pool,
                EthEvmConfig::new(ctx.provider().chain_spec().clone()),
                EthereumBuilderConfig::new(),
            ),
        };
        Ok(payload_builder)
    }
}

/// The type responsible for building custom payloads
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StrataPayloadBuilder<Pool, Client> {
    inner: reth_ethereum_payload_builder::EthereumPayloadBuilder<Pool, Client>,
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
        let BuildArguments {
            cached_reads,
            config,
            cancel,
            best_payload,
        } = args;
        let PayloadConfig {
            parent_header,
            attributes,
        } = config;

        // This reuses the default EthereumPayloadBuilder to build the payload
        // but any custom logic can be implemented here
        // let res = self.inner.try_build(BuildArguments {
        //     cached_reads,
        //     config: PayloadConfig {
        //         parent_header,
        //         attributes: attributes.inner,
        //     },
        //     cancel,
        //     best_payload,
        // });

        todo!()
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
        // let eth_build_payload = <reth_ethereum_payload_builder::EthereumPayloadBuilder<
        //     Pool,
        //     Client,
        // > as PayloadBuilder>::build_empty_payload(
        // > &reth_ethereum_payload_builder::EthereumPayloadBuilder::new( self.client.clone(),
        // > self.pool.clone(), self.evm_config.inner().clone(), self.builder_config.clone(), ),
        // > PayloadConfig { parent_header, attributes: attributes.inner, },
        // )?;
        todo!()
    }
}
