//! Loads and formats Strata transaction RPC response.

use alloy_consensus::{Signed, Transaction as _, TxEnvelope};
use alloy_primitives::{Bytes, PrimitiveSignature as Signature, B256};
use alloy_rpc_types_eth::{Transaction, TransactionInfo, TransactionRequest};
use reth_node_api::FullNodeComponents;
use reth_primitives::{RecoveredTx, TransactionSigned};
use reth_provider::{
    BlockReader, BlockReaderIdExt, ProviderTx, ReceiptProvider, TransactionsProvider,
};
use reth_rpc_eth_api::{
    helpers::{EthSigner, EthTransactions, LoadTransaction, SpawnBlocking},
    FromEthApiError, FullEthApiTypes, RpcNodeCore, RpcNodeCoreExt, TransactionCompat,
};
use reth_rpc_eth_types::{utils::recover_raw_transaction, EthApiError};
use reth_transaction_pool::{PoolTransaction, TransactionOrigin, TransactionPool};

use crate::{SequencerClient, StrataEthApi, StrataNodeCore};

impl<N> EthTransactions for StrataEthApi<N>
where
    Self: LoadTransaction<Provider: BlockReaderIdExt>,
    N: StrataNodeCore<Provider: BlockReader<Transaction = ProviderTx<Self::Provider>>>,
{
    fn signers(&self) -> &parking_lot::RwLock<Vec<Box<dyn EthSigner<ProviderTx<Self::Provider>>>>> {
        self.inner.eth_api.signers()
    }

    /// Decodes and recovers the transaction and submits it to the pool.
    ///
    /// Returns the hash of the transaction.
    async fn send_raw_transaction(&self, tx: Bytes) -> Result<B256, Self::Error> {
        let recovered = recover_raw_transaction(&tx)?;
        let pool_transaction = <Self::Pool as TransactionPool>::Transaction::from_pooled(recovered);

        // On Strata, transactions are forwarded directly to the sequencer to be included in
        // blocks that it builds.
        if let Some(client) = self.raw_tx_forwarder().as_ref() {
            tracing::debug!( target: "rpc::eth",  "forwarding raw transaction to");
            let _ = client.forward_raw_transaction(&tx).await.inspect_err(|err| {
                    tracing::debug!(target: "rpc::eth", %err, hash=% *pool_transaction.hash(), "failed to forward raw transaction");
                });
        }

        // submit the transaction to the pool with a `Local` origin
        let hash = self
            .pool()
            .add_transaction(TransactionOrigin::Local, pool_transaction)
            .await
            .map_err(Self::Error::from_eth_err)?;

        Ok(hash)
    }
}

impl<N> LoadTransaction for StrataEthApi<N>
where
    Self: SpawnBlocking + FullEthApiTypes + RpcNodeCoreExt,
    N: StrataNodeCore<Provider: TransactionsProvider, Pool: TransactionPool>,
    Self::Pool: TransactionPool,
{
}

impl<N> StrataEthApi<N>
where
    N: StrataNodeCore,
{
    /// Returns the [`SequencerClient`] if one is set.
    pub fn raw_tx_forwarder(&self) -> Option<SequencerClient> {
        self.inner.sequencer_client.clone()
    }
}

impl<N> TransactionCompat<TransactionSigned> for StrataEthApi<N>
where
    N: FullNodeComponents<Provider: ReceiptProvider<Receipt = reth_primitives::Receipt>>,
{
    type Transaction = Transaction;
    type Error = EthApiError;

    fn fill(
        &self,
        tx: RecoveredTx<TransactionSigned>,
        tx_info: TransactionInfo,
    ) -> Result<Self::Transaction, Self::Error> {
        let from = tx.signer();
        let hash = tx.hash();
        let TransactionSigned {
            transaction,
            signature,
            ..
        } = tx.into_tx();

        let inner = match transaction {
            reth_primitives::Transaction::Legacy(tx) => {
                TxEnvelope::Legacy(Signed::new_unchecked(tx, signature, hash))
            }
            reth_primitives::Transaction::Eip2930(tx) => {
                TxEnvelope::Eip2930(Signed::new_unchecked(tx, signature, hash))
            }
            reth_primitives::Transaction::Eip1559(tx) => {
                TxEnvelope::Eip1559(Signed::new_unchecked(tx, signature, hash))
            }
            reth_primitives::Transaction::Eip4844(_) => unreachable!(),
            reth_primitives::Transaction::Eip7702(tx) => {
                TxEnvelope::Eip7702(Signed::new_unchecked(tx, signature, hash))
            }
        };

        let TransactionInfo {
            block_hash,
            block_number,
            index: transaction_index,
            base_fee,
            ..
        } = tx_info;

        let effective_gas_price = base_fee
            .map(|base_fee| {
                inner
                    .effective_tip_per_gas(base_fee as u64)
                    .unwrap_or_default()
                    + base_fee
            })
            .unwrap_or_else(|| inner.max_fee_per_gas());

        Ok(Transaction {
            inner,
            block_hash,
            block_number,
            transaction_index,
            from,
            effective_gas_price: Some(effective_gas_price),
        })
    }

    fn build_simulate_v1_transaction(
        &self,
        request: TransactionRequest,
    ) -> Result<TransactionSigned, Self::Error> {
        let Ok(tx) = request.build_typed_tx() else {
            return Err(EthApiError::TransactionConversionError);
        };

        // Create an empty signature for the transaction.
        let signature = Signature::new(Default::default(), Default::default(), false);
        Ok(TransactionSigned::new_unhashed(tx.into(), signature))
    }

    fn otterscan_api_truncate_input(tx: &mut Self::Transaction) {
        let input = match &mut tx.inner {
            TxEnvelope::Eip1559(tx) => &mut tx.tx_mut().input,
            TxEnvelope::Eip2930(tx) => &mut tx.tx_mut().input,
            TxEnvelope::Legacy(tx) => &mut tx.tx_mut().input,
            TxEnvelope::Eip7702(tx) => &mut tx.tx_mut().input,
            _ => return,
        };
        *input = input.slice(..4);
    }
}
