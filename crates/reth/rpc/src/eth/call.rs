use reth_chainspec::ChainSpec;
use reth_evm::ConfigureEvm;
use reth_node_api::{FullNodeComponents, NodeTypes};
use reth_primitives::Header;
use reth_rpc_eth_api::helpers::{Call, EthCall, LoadState, SpawnBlocking};
use reth_rpc_eth_types::EthApiError;

use crate::StrataEthApi;

impl<N> EthCall for StrataEthApi<N>
where
    Self: Call,
    N: FullNodeComponents<Types: NodeTypes<ChainSpec = ChainSpec>>,
{
}

impl<N> Call for StrataEthApi<N>
where
    Self: LoadState + SpawnBlocking,
    Self::Error: From<EthApiError>,
    N: FullNodeComponents,
{
    #[inline]
    fn call_gas_limit(&self) -> u64 {
        self.inner.gas_cap()
    }

    #[inline]
    fn max_simulate_blocks(&self) -> u64 {
        self.inner.max_simulate_blocks()
    }

    #[inline]
    fn evm_config(&self) -> &impl ConfigureEvm<Header = Header> {
        self.inner.evm_config()
    }
}
