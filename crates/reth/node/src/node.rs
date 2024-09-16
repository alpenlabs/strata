use express_reth_evm::ExpressEvmConfig;
use reth::builder::{
    components::{ComponentsBuilder, ExecutorBuilder},
    BuilderContext, Node,
};
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_ethereum::{
    node::{EthereumAddOns, EthereumConsensusBuilder, EthereumNetworkBuilder, EthereumPoolBuilder},
    EthExecutorProvider,
};

use super::{engine::ExpressEngineTypes, payload_builder::ExpressPayloadServiceBuilder};

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ExpressEthereumNode;

/// Configure the node types
impl NodeTypes for ExpressEthereumNode {
    type Primitives = ();
    // use the custom engine types
    type Engine = ExpressEngineTypes;
}

/// Implement the Node trait for the custom node
///
/// This provides a preset configuration for the node
impl<N> Node<N> for ExpressEthereumNode
where
    N: FullNodeTypes<Engine = ExpressEngineTypes>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        EthereumPoolBuilder,
        ExpressPayloadServiceBuilder,
        EthereumNetworkBuilder,
        ExpressExecutorBuilder,
        EthereumConsensusBuilder,
    >;
    type AddOns = EthereumAddOns;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        ComponentsBuilder::default()
            .node_types::<N>()
            .pool(EthereumPoolBuilder::default())
            .payload(ExpressPayloadServiceBuilder::default())
            .network(EthereumNetworkBuilder::default())
            .executor(ExpressExecutorBuilder::default())
            .consensus(EthereumConsensusBuilder::default())
    }
}

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct ExpressExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for ExpressExecutorBuilder
where
    Node: FullNodeTypes,
{
    type EVM = ExpressEvmConfig;
    type Executor = EthExecutorProvider<Self::EVM>;

    async fn build_evm(
        self,
        ctx: &BuilderContext<Node>,
    ) -> eyre::Result<(Self::EVM, Self::Executor)> {
        Ok((
            ExpressEvmConfig::default(),
            EthExecutorProvider::new(ctx.chain_spec(), ExpressEvmConfig::default()),
        ))
    }
}
