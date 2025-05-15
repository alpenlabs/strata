use std::sync::Arc;

use reth_chainspec::ChainSpec;
use reth_evm::{
    block::{BlockExecutorFactory, BlockExecutorFor},
    env::EvmEnv,
    eth::{EthBlockExecutionCtx, EthBlockExecutor},
    ConfigureEvm, Database, EthEvmFactory, InspectorFor, NextBlockEnvAttributes,
};
use reth_evm_ethereum::{EthBlockAssembler, EthEvm, EthEvmConfig};
use reth_primitives::{Header, Receipt, SealedBlock, SealedHeader, TransactionSigned};
use revm::database::State;
use revm_primitives::hardfork::SpecId;
// use strata_reth_evm::set_evm_handles;

/// Custom EVM configuration
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StrataEvmConfig {
    inner: EthEvmConfig,
}

impl StrataEvmConfig {
    pub fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            inner: EthEvmConfig::new(chain_spec),
        }
    }

    pub fn inner(&self) -> &EthEvmConfig {
        &self.inner
    }
}

impl BlockExecutorFactory for StrataEvmConfig {
    type EvmFactory = EthEvmFactory;
    type ExecutionCtx<'a> = EthBlockExecutionCtx<'a>;
    type Transaction = TransactionSigned;
    type Receipt = Receipt;

    fn evm_factory(&self) -> &Self::EvmFactory {
        self.inner.evm_factory()
    }

    fn create_executor<'a, DB, I>(
        &'a self,
        evm: EthEvm<&'a mut State<DB>, I>,
        ctx: EthBlockExecutionCtx<'a>,
    ) -> impl BlockExecutorFor<'a, Self, DB, I>
    where
        DB: Database + 'a,
        I: InspectorFor<Self, &'a mut State<DB>> + 'a,
    {
        EthBlockExecutor::new(
            evm,
            ctx,
            self.inner.chain_spec(),
            self.inner.executor_factory.receipt_builder(),
        )
    }
}

impl ConfigureEvm for StrataEvmConfig {
    type Primitives = <EthEvmConfig as ConfigureEvm>::Primitives;
    type Error = <EthEvmConfig as ConfigureEvm>::Error;
    type NextBlockEnvCtx = <EthEvmConfig as ConfigureEvm>::NextBlockEnvCtx;
    type BlockExecutorFactory = Self;
    type BlockAssembler = EthBlockAssembler<ChainSpec>;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        self
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        self.inner.block_assembler()
    }

    fn evm_env(&self, header: &Header) -> EvmEnv<SpecId> {
        self.inner.evm_env(header)
    }

    fn next_evm_env(
        &self,
        parent: &Header,
        attributes: &NextBlockEnvAttributes,
    ) -> Result<EvmEnv<SpecId>, Self::Error> {
        self.inner.next_evm_env(parent, attributes)
    }

    fn context_for_block<'a>(&self, block: &'a SealedBlock) -> EthBlockExecutionCtx<'a> {
        self.inner.context_for_block(block)
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader,
        attributes: Self::NextBlockEnvCtx,
    ) -> EthBlockExecutionCtx<'_> {
        self.inner.context_for_next_block(parent, attributes)
    }
}
