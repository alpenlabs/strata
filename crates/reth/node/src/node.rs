use reth::builder::{components::ComponentsBuilder, Node};
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_ethereum::node::{
    EthereumAddOns, EthereumConsensusBuilder, EthereumExecutorBuilder, EthereumNetworkBuilder,
    EthereumPoolBuilder,
};

use super::{engine::CustomEngineTypes, payload_builder::ExpressPayloadServiceBuilder};

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ExpressEthereumNode;

/// Configure the node types
impl NodeTypes for ExpressEthereumNode {
    type Primitives = ();
    // use the custom engine types
    type Engine = CustomEngineTypes;
}

/// Implement the Node trait for the custom node
///
/// This provides a preset configuration for the node
impl<N> Node<N> for ExpressEthereumNode
where
    N: FullNodeTypes<Engine = CustomEngineTypes>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        EthereumPoolBuilder,
        ExpressPayloadServiceBuilder,
        EthereumNetworkBuilder,
        EthereumExecutorBuilder,
        EthereumConsensusBuilder,
    >;
    type AddOns = EthereumAddOns;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        ComponentsBuilder::default()
            .node_types::<N>()
            .pool(EthereumPoolBuilder::default())
            .payload(ExpressPayloadServiceBuilder::default())
            .network(EthereumNetworkBuilder::default())
            .executor(EthereumExecutorBuilder::default())
            .consensus(EthereumConsensusBuilder::default())
    }
}
