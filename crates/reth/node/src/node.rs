use alloy_consensus::Header;
use reth::rpc::eth::{core::EthApiFor, FullEthApiServer};
use reth_chainspec::{ChainSpec, EthereumHardforks};
use reth_db::transaction::{DbTx, DbTxMut};
use reth_evm::{ConfigureEvm, NextBlockEnvAttributes};
use reth_node_api::{AddOnsContext, FullNodeComponents, NodeAddOns};
use reth_node_builder::{
    components::{BasicPayloadServiceBuilder, ComponentsBuilder, ExecutorBuilder},
    node::{FullNodeTypes, NodeTypes},
    rpc::{EngineValidatorAddOn, EthApiBuilder, RethRpcAddOns, RpcAddOns, RpcHandle},
    BuilderContext, Node, NodeAdapter, NodeComponentsBuilder,
};
use reth_node_ethereum::{
    node::{
        EthereumConsensusBuilder, EthereumEngineValidatorBuilder, EthereumNetworkBuilder,
        EthereumPoolBuilder,
    },
    BasicBlockExecutorProvider, EthereumEthApiBuilder,
};
use reth_primitives::BlockBody;
use reth_provider::{
    providers::{ChainStorage, NodeTypesForProvider},
    BlockBodyReader, BlockBodyWriter, ChainSpecProvider, ChainStorageReader, ChainStorageWriter,
    DBProvider, DatabaseProvider, EthStorage, OmmersProvider, ProviderResult, ReadBodyInput,
    StorageLocation,
};
use reth_rpc_eth_types::{error::FromEvmError, EthApiError};
use revm_primitives::alloy_primitives;
use strata_reth_rpc::{eth::StrataEthApiBuilder, SequencerClient, StrataEthApi};

use crate::{
    args::StrataNodeArgs,
    engine::{StrataEngineTypes, StrataEngineValidator, StrataEngineValidatorBuilder},
    evm::StrataEvmConfig,
    payload_builder::StrataPayloadServiceBuilder,
};

/// Strata primitive types.
pub(crate) type StrataPrimitives = reth_primitives::EthPrimitives;

/// Storage implementation for Strata.
#[derive(Debug, Default, Clone)]
pub struct StrataStorage(EthStorage);

impl<Provider: DBProvider<Tx: DbTxMut>> BlockBodyWriter<Provider, BlockBody> for StrataStorage {
    fn write_block_bodies(
        &self,
        provider: &Provider,
        bodies: Vec<(u64, Option<BlockBody>)>,
        write_to: StorageLocation,
    ) -> ProviderResult<()> {
        self.0.write_block_bodies(provider, bodies, write_to)
    }

    fn remove_block_bodies_above(
        &self,
        provider: &Provider,
        block: alloy_primitives::BlockNumber,
        remove_from: StorageLocation,
    ) -> ProviderResult<()> {
        self.0
            .remove_block_bodies_above(provider, block, remove_from)
    }
}

impl<
        Provider: DBProvider
            + ChainSpecProvider<ChainSpec: EthereumHardforks>
            + OmmersProvider<Header = Header>,
    > BlockBodyReader<Provider> for StrataStorage
{
    type Block = reth_primitives::Block;

    fn read_block_bodies(
        &self,
        provider: &Provider,
        inputs: Vec<ReadBodyInput<'_, Self::Block>>,
    ) -> ProviderResult<Vec<BlockBody>> {
        self.0.read_block_bodies(provider, inputs)
    }
}

impl ChainStorage<StrataPrimitives> for StrataStorage {
    fn reader<TX, Types>(
        &self,
    ) -> impl ChainStorageReader<DatabaseProvider<TX, Types>, StrataPrimitives>
    where
        TX: DbTx + 'static,
        Types: NodeTypesForProvider<Primitives = StrataPrimitives>,
    {
        self
    }

    fn writer<TX, Types>(
        &self,
    ) -> impl ChainStorageWriter<DatabaseProvider<TX, Types>, StrataPrimitives>
    where
        TX: DbTxMut + DbTx + 'static,
        Types: NodeTypes<Primitives = StrataPrimitives>,
    {
        self
    }
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct StrataEthereumNode {
    // Strata node args.
    pub args: StrataNodeArgs,
}
