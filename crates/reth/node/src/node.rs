use reth::builder::{
    components::{ComponentsBuilder, ExecutorBuilder},
    BuilderContext, Node,
};
use reth_chainspec::ChainSpec;
use reth_node_api::{FullNodeTypes, NodeTypes, NodeTypesWithEngine};
use reth_node_ethereum::{
    node::{EthereumAddOns, EthereumConsensusBuilder, EthereumNetworkBuilder, EthereumPoolBuilder},
    EthExecutorProvider,
};

use crate::{
    engine::StrataEngineTypes, evm::StrataEvmConfig, payload_builder::StrataPayloadServiceBuilder,
    validator::StrataEngineValidatorBuilder,
};

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct StrataEthereumNode;

/// Configure the node types
impl NodeTypes for StrataEthereumNode {
    type Primitives = ();
    type ChainSpec = ChainSpec;
}

/// Configure the node types with the custom engine types
impl NodeTypesWithEngine for StrataEthereumNode {
    // use the custom engine types
    type Engine = StrataEngineTypes;
}

/// Implement the Node trait for the custom node
///
/// This provides a preset configuration for the node
impl<N> Node<N> for StrataEthereumNode
where
    N: FullNodeTypes<Types: NodeTypesWithEngine<Engine = StrataEngineTypes, ChainSpec = ChainSpec>>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        EthereumPoolBuilder,
        StrataPayloadServiceBuilder,
        EthereumNetworkBuilder,
        StrataExecutorBuilder,
        EthereumConsensusBuilder,
        StrataEngineValidatorBuilder,
    >;
    type AddOns = EthereumAddOns;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        ComponentsBuilder::default()
            .node_types::<N>()
            .pool(EthereumPoolBuilder::default())
            .payload(StrataPayloadServiceBuilder::default())
            .network(EthereumNetworkBuilder::default())
            .executor(StrataExecutorBuilder::default())
            .consensus(EthereumConsensusBuilder::default())
            .engine_validator(StrataEngineValidatorBuilder::default())
    }

    fn add_ons(&self) -> Self::AddOns {
        EthereumAddOns::default()
    }
}

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct StrataExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for StrataExecutorBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec>>,
{
    type EVM = StrataEvmConfig;
    type Executor = EthExecutorProvider<Self::EVM>;

    async fn build_evm(
        self,
        ctx: &BuilderContext<Node>,
    ) -> eyre::Result<(Self::EVM, Self::Executor)> {
        Ok((
            StrataEvmConfig::new(ctx.chain_spec()),
            EthExecutorProvider::new(ctx.chain_spec(), StrataEvmConfig::new(ctx.chain_spec())),
        ))
    }
}
