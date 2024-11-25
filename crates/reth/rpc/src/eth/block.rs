//! Loads and formats OP block RPC response.

use alloy_rpc_types::{AnyTransactionReceipt, BlockId};
use reth_chainspec::ChainSpec;
use reth_node_api::{FullNodeComponents, NodeTypes};
use reth_primitives::TransactionMeta;
use reth_provider::{BlockReaderIdExt, HeaderProvider};
use reth_rpc_eth_api::{
    helpers::{EthBlocks, LoadBlock, LoadPendingBlock, LoadReceipt, SpawnBlocking},
    RpcReceipt,
};
use reth_rpc_eth_types::{EthApiError, EthStateCache, ReceiptBuilder};

use crate::StrataEthApi;

impl<N> EthBlocks for StrataEthApi<N>
where
    Self: LoadBlock<
        Error = EthApiError,
        NetworkTypes: alloy_network::Network<ReceiptResponse = AnyTransactionReceipt>,
    >,
    N: FullNodeComponents<Types: NodeTypes<ChainSpec = ChainSpec>>,
{
    #[inline]
    fn provider(&self) -> impl HeaderProvider {
        self.inner.provider()
    }

    async fn block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<RpcReceipt<Self::NetworkTypes>>>, Self::Error>
    where
        Self: LoadReceipt,
    {
        if let Some((block, receipts)) = self.load_block_and_receipts(block_id).await? {
            let block_number = block.number;
            let base_fee = block.base_fee_per_gas;
            let block_hash = block.hash();
            let excess_blob_gas = block.excess_blob_gas;
            let timestamp = block.timestamp;
            let block = block.unseal();

            return block
                .body
                .transactions
                .into_iter()
                .zip(receipts.iter())
                .enumerate()
                .map(|(idx, (tx, receipt))| {
                    let meta = TransactionMeta {
                        tx_hash: tx.hash,
                        index: idx as u64,
                        block_hash,
                        block_number,
                        base_fee,
                        excess_blob_gas,
                        timestamp,
                    };

                    ReceiptBuilder::new(&tx, meta, receipt, &receipts)
                        .map(|builder| builder.build())
                })
                .collect::<Result<Vec<_>, Self::Error>>()
                .map(Some);
        }

        Ok(None)
    }
}

impl<N> LoadBlock for StrataEthApi<N>
where
    Self: LoadPendingBlock + SpawnBlocking,
    N: FullNodeComponents,
{
    #[inline]
    fn provider(&self) -> impl BlockReaderIdExt {
        self.inner.provider()
    }

    #[inline]
    fn cache(&self) -> &EthStateCache {
        self.inner.cache()
    }
}
