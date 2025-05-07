use std::sync::Arc;

use alpen_reth_evm::set_evm_handles;
use reth::revm::primitives::EVMError;
use reth_chainspec::ChainSpec;
use reth_evm::{
    env::EvmEnv, ConfigureEvm, ConfigureEvmEnv, Database, EthEvmFactory, NextBlockEnvAttributes,
};
use reth_evm_ethereum::{EthEvm, EthEvmConfig};
use reth_primitives::{Header, TransactionSigned};
use revm::{
    handler::EthPrecompiles, inspector_handle_register, Context, Evm, EvmBuilder, MainContext,
};
use revm_primitives::{Address, CfgEnvWithHandlerCfg, HaltReason, HandlerCfg, TxEnv};

#[derive(Clone)]
pub struct StrataPrecompiles {
    pub precompiles: EthPrecompiles,
}

impl StrataPrecompiles {
    fn new() -> Self {
        Self {
            precompiles: EthPrecompiles::default(),
        }
    }
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for StrataPrecompiles {
    // TODO(QQ): fix precompiles.
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        let spec_id = spec.clone().into();
        if spec_id == SpecId::PRAGUE {
            //self.precompiles = EthPrecompiles {
            //    precompiles: prague_custom(),
            //    spec: spec.into(),
            //}
        } else {
            PrecompileProvider::<CTX>::set_spec(&mut self.precompiles, spec);
        }
        true
    }

    fn run(
        &mut self,
        context: &mut CTX,
        address: &Address,
        inputs: &InputsImpl,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, String> {
        let Some(precompile) = self.precompiles.get(address) else {
            return Ok(None);
        };

        let mut result = InterpreterResult {
            result: InstructionResult::Return,
            gas: Gas::new(gas_limit),
            output: Bytes::new(),
        };

        match (*precompile)(&inputs.input, context, gas_limit) {
            Ok(output) => {
                let underflow = result.gas.record_cost(output.gas_used);
                assert!(underflow, "Gas underflow is not possible");
                result.result = InstructionResult::Return;
                result.output = output.bytes;
            }
            Err(PrecompileError::Fatal(e)) => return Err(e),
            Err(e) => {
                result.result = if e.is_oog() {
                    InstructionResult::PrecompileOOG
                } else {
                    InstructionResult::PrecompileError
                };
            }
        }
        Ok(Some(result))
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.precompiles.warm_addresses()
    }

    fn contains(&self, address: &Address) -> bool {
        self.precompiles.contains(address)
    }
}

/// Custom EVM Factory
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct StrataEvmFactory;

impl EvmFactory for StrataEvmFactory {
    type Evm<DB: Database, I: Inspector<EthEvmContext<DB>>> = EthEvm<DB, I>;
    type Context<DB: Database> = Context<BlockEnv, TxEnv, CfgEnv, DB>;
    type Tx = TxEnv;
    type Error<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    type HaltReason = HaltReason;
    type Spec = SpecId;

    fn create_evm<DB: Database>(&self, db: DB, input: EvmEnv) -> Self::Evm<DB, NoOpInspector> {
        let evm = Context::mainnet()
            .with_db(db)
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .with_precompiles(StrataPrecompiles::new())
            .build_mainnet_with_inspector(NoOpInspector {});

        EthEvm::new(evm, false)
    }

    fn create_evm_with_inspector<DB: Database, I: Inspector<Self::Context<DB>, EthInterpreter>>(
        &self,
        db: DB,
        input: EvmEnv,
        inspector: I,
    ) -> Self::Evm<DB, I> {
        EthEvm::new(
            self.create_evm(db, input)
                .into_inner()
                .with_inspector(inspector),
            true,
        )
    }
}

/// Custom EVM configuration
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StrataEvmConfig {
    inner: EthEvmConfig<StrataEvmFactory>,
}

impl StrataEvmConfig {
    pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self {
            inner: EthEvmConfig::new(chain_spec),
        }
    }

    pub fn inner(&self) -> &EthEvmConfig {
        &self.inner
    }
}

impl ConfigureEvm for StrataEvmConfig {
    type Primitives = EthPrimitives;
    type Error = Infallible;
    type NextBlockEnvCtx = NextBlockEnvAttributes;
    type BlockExecutorFactory =
        EthBlockExecutorFactory<RethReceiptBuilder, Arc<ChainSpec>, StrataEvmFactory>;
    type BlockAssembler = EthBlockAssembler<ChainSpec>;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        self.inner().block_executor_factory()
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        self.inner().block_assembler()
    }

    fn evm_env(&self, header: &HeaderTy<Self::Primitives>) -> EvmEnvFor<Self> {
        self.inner().evm_env(header)
    }

    fn next_evm_env(
        &self,
        parent: &HeaderTy<Self::Primitives>,
        attributes: &Self::NextBlockEnvCtx,
    ) -> Result<EvmEnvFor<Self>, Self::Error> {
        self.inner().next_evm_env(parent, attributes)
    }

    fn context_for_block<'a>(
        &self,
        block: &'a SealedBlock<BlockTy<Self::Primitives>>,
    ) -> ExecutionCtxFor<'a, Self> {
        self.inner().context_for_block(block)
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader<HeaderTy<Self::Primitives>>,
        attributes: Self::NextBlockEnvCtx,
    ) -> ExecutionCtxFor<'_, Self> {
        self.inner().context_for_next_block(parent, attributes)
    }
}
