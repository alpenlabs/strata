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
        EthereumAddOns, EthereumConsensusBuilder, EthereumEngineValidatorBuilder,
        EthereumExecutorBuilder, EthereumNetworkBuilder, EthereumPayloadBuilder,
        EthereumPoolBuilder,
    },
    BasicBlockExecutorProvider, EthEngineTypes, EthereumEthApiBuilder,
};
use reth_primitives::{BlockBody, EthPrimitives};
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
    args::StrataNodeArgs, evm::StrataEvmConfig, payload_builder::StrataPayloadServiceBuilder,
    StrataPayloadTypes,
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

impl StrataEthereumNode {
    /// Creates a new instance of the StrataEthereum node type.
    pub fn new(args: StrataNodeArgs) -> Self {
        Self { args }
    }

    /// Returns a [`ComponentsBuilder`] configured for a regular Ethereum node.
    pub fn components<Node>() -> ComponentsBuilder<
        Node,
        EthereumPoolBuilder,
        BasicPayloadServiceBuilder<EthereumPayloadBuilder>,
        EthereumNetworkBuilder,
        EthereumExecutorBuilder,
        EthereumConsensusBuilder,
    >
    where
        Node: FullNodeTypes<
            Types: NodeTypes<
                Payload = EthEngineTypes,
                ChainSpec = ChainSpec,
                Primitives = StrataPrimitives,
            >,
        >,
    {
        ComponentsBuilder::default()
            .node_types::<Node>()
            .pool(EthereumPoolBuilder::default())
            .payload(BasicPayloadServiceBuilder::default())
            .network(EthereumNetworkBuilder::default())
            .executor(EthereumExecutorBuilder::default())
            .consensus(EthereumConsensusBuilder::default())
    }
}

impl NodeTypes for StrataEthereumNode {
    type Primitives = StrataPrimitives;
    type ChainSpec = ChainSpec;
    type StateCommitment = reth_trie_db::MerklePatriciaTrie;
    type Storage = StrataStorage;
    type Payload = EthEngineTypes;
}

impl<N> Node<N> for StrataEthereumNode
where
    N: FullNodeTypes<
        Types: NodeTypes<
            Payload = EthEngineTypes,
            ChainSpec = ChainSpec,
            Primitives = StrataPrimitives,
            Storage = StrataStorage,
        >,
    >,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        EthereumPoolBuilder,
        BasicPayloadServiceBuilder<EthereumPayloadBuilder>,
        EthereumNetworkBuilder,
        EthereumExecutorBuilder,
        EthereumConsensusBuilder,
    >;

    type AddOns = EthereumAddOns<
        NodeAdapter<N, <Self::ComponentsBuilder as NodeComponentsBuilder<N>>::Components>,
    >;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        Self::components()
    }

    fn add_ons(&self) -> Self::AddOns {
        Self::AddOns::default()
    }
}

// #[derive(Debug, Default, Clone)]
// pub struct StrataAddOns<N: FullNodeComponents> {
//     pub rpc_add_ons: RpcAddOns<N, StrataEthApiBuilder, StrataEngineValidatorBuilder>,
// }

// #[derive(Debug, Default, Clone)]
// #[non_exhaustive]
// pub struct StrataAddOnsBuilder {
//     /// Sequencer client, configured to forward submitted transactions to sequencer of given OP
//     /// network.
//     sequencer_client: Option<SequencerClient>,
// }

// impl StrataAddOnsBuilder {
//     /// With a [`SequencerClient`].
//     pub fn with_sequencer(mut self, sequencer_client: Option<String>) -> Self {
//         self.sequencer_client = sequencer_client.map(SequencerClient::new);
//         self
//     }
// }

// impl StrataAddOnsBuilder {
//     /// Builds an instance of [`StrataAddOns`].
//     pub fn build<N>(self) -> StrataAddOns<N>
//     where
//         N: FullNodeComponents<Types: NodeTypes<Primitives = StrataPrimitives>>,
//     {
//         let Self { sequencer_client } = self;

//         StrataAddOns {
//             rpc_add_ons: RpcAddOns::new(
//                 move |ctx| {
//                     StrataEthApi::<N>::builder()
//                         .with_sequencer(sequencer_client)
//                         .build(ctx)
//                 },
//                 Default::default(),
//                 Default::default(),
//             ),
//         }
//     }
// }
