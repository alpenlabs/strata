//! Loads OP pending block for a RPC response.

use reth_chainspec::{ChainSpec, EthereumHardforks};
use reth_evm::ConfigureEvm;
use reth_node_api::{FullNodeComponents, NodeTypes};
use reth_primitives::Header;
use reth_provider::{BlockReaderIdExt, ChainSpecProvider, EvmEnvProvider, StateProviderFactory};
use reth_rpc_eth_api::helpers::LoadPendingBlock;
use reth_rpc_eth_types::PendingBlock;
use reth_transaction_pool::TransactionPool;

use crate::StrataEthApi;

impl<N> LoadPendingBlock for StrataEthApi<N>
where
    Self: Send + Sync,
    N: FullNodeComponents<Types: NodeTypes<ChainSpec = ChainSpec>>,
{
    #[inline]
    fn provider(
        &self,
    ) -> impl BlockReaderIdExt
           + EvmEnvProvider
           + ChainSpecProvider<ChainSpec: EthereumHardforks>
           + StateProviderFactory {
        self.inner.provider()
    }

    #[inline]
    fn pool(&self) -> impl TransactionPool {
        self.inner.pool()
    }

    #[inline]
    fn pending_block(&self) -> &tokio::sync::Mutex<Option<PendingBlock>> {
        self.inner.pending_block()
    }

    #[inline]
    fn evm_config(&self) -> &impl ConfigureEvm<Header = Header> {
        self.inner.evm_config()
    }
}
