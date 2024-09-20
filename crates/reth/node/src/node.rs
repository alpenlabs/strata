use reth::builder::{
    components::{ComponentsBuilder, ExecutorBuilder},
    BuilderContext, Node,
};
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_ethereum::{
    node::{EthereumAddOns, EthereumConsensusBuilder, EthereumNetworkBuilder, EthereumPoolBuilder},
    EthExecutorProvider,
};

use crate::{
    engine::StrataEngineTypes, evm::StrataEvmConfig, payload_builder::StrataPayloadServiceBuilder,
};

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct StrataEthereumNode;

/// Configure the node types
impl NodeTypes for StrataEthereumNode {
    type Primitives = ();
    // use the custom engine types
    type Engine = StrataEngineTypes;
}

/// Implement the Node trait for the custom node
///
/// This provides a preset configuration for the node
impl<N> Node<N> for StrataEthereumNode
where
    N: FullNodeTypes<Engine = StrataEngineTypes>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        EthereumPoolBuilder,
        StrataPayloadServiceBuilder,
        EthereumNetworkBuilder,
        StrataExecutorBuilder,
        EthereumConsensusBuilder,
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
    }
}

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct StrataExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for StrataExecutorBuilder
where
    Node: FullNodeTypes,
{
    type EVM = StrataEvmConfig;
    type Executor = EthExecutorProvider<Self::EVM>;

    async fn build_evm(
        self,
        ctx: &BuilderContext<Node>,
    ) -> eyre::Result<(Self::EVM, Self::Executor)> {
        Ok((
            StrataEvmConfig::default(),
            EthExecutorProvider::new(ctx.chain_spec(), StrataEvmConfig::default()),
        ))
    }
}
