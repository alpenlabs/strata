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
    args::StrataNodeArgs, evm::StrataEvmConfig, payload_builder::StrataPayloadBuilderBuilder,
};

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
}

impl NodeTypes for StrataEthereumNode {
    type Primitives = EthPrimitives;
    type ChainSpec = ChainSpec;
    type StateCommitment = reth_trie_db::MerklePatriciaTrie;
    type Storage = EthStorage;
    type Payload = EthEngineTypes;
}

impl<N> Node<N> for StrataEthereumNode
where
    N: FullNodeTypes<
        Types: NodeTypes<
            Payload = EthEngineTypes,
            ChainSpec = ChainSpec,
            Primitives = EthPrimitives,
            Storage = EthStorage,
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
        ComponentsBuilder::default()
            .node_types::<N>()
            .pool(EthereumPoolBuilder::default())
            .payload(BasicPayloadServiceBuilder::default())
            .network(EthereumNetworkBuilder::default())
            .executor(EthereumExecutorBuilder::default())
            .consensus(EthereumConsensusBuilder::default())
    }

    fn add_ons(&self) -> Self::AddOns {
        Self::AddOns::default()
    }
}
