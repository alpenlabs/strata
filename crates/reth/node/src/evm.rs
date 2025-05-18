use std::sync::OnceLock;

use reth_chainspec::ChainSpec;
use reth_evm::{eth::EthEvmContext, EthEvm, EvmEnv, EvmFactory};
use reth_evm_ethereum::EthEvmConfig;
use reth_node_api::{FullNodeTypes, NodeTypes};
use reth_node_builder::{components::ExecutorBuilder, BuilderContext};
use reth_node_ethereum::BasicBlockExecutorProvider;
use reth_primitives::EthPrimitives;
use revm::{
    context::{
        result::{EVMError, HaltReason},
        Cfg, ContextSetters, ContextTr, Evm, EvmData, TxEnv,
    },
    handler::{
        instructions::{EthInstructions, InstructionProvider},
        EthPrecompiles, EvmTr, PrecompileProvider,
    },
    inspector::{inspect_instructions, InspectorEvmTr, JournalExt, NoOpInspector},
    interpreter::{
        interpreter::EthInterpreter, InputsImpl, Interpreter, InterpreterResult, InterpreterTypes,
    },
    precompile::{PrecompileFn, PrecompileOutput, PrecompileResult, Precompiles},
    Context, Inspector, MainBuilder, MainContext,
};
use revm_primitives::{address, hardfork::SpecId, Address, Bytes};

/// A custom precompile that contains static precompiles.
#[derive(Clone)]
pub struct CustomPrecompiles {
    pub precompiles: EthPrecompiles,
}

impl CustomPrecompiles {
    /// Given a [`PrecompileProvider`] and cache for a specific precompiles, create a
    /// wrapper that can be used inside Evm.
    fn new() -> Self {
        Self {
            precompiles: EthPrecompiles::default(),
        }
    }
}

/// Returns precompiles for Fjor spec.
pub fn prague_custom() -> &'static Precompiles {
    static INSTANCE: OnceLock<Precompiles> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = Precompiles::prague().clone();
        // Custom precompile.
        precompiles.extend([(
            address!("0x0000000000000000000000000000000000000999"),
            |_, _| -> PrecompileResult {
                PrecompileResult::Ok(PrecompileOutput::new(0, Bytes::new()))
            } as PrecompileFn,
        )
            .into()]);
        precompiles
    })
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for CustomPrecompiles {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        let spec_id = spec.clone().into();
        if spec_id == SpecId::PRAGUE {
            self.precompiles = EthPrecompiles {
                precompiles: prague_custom(),
                spec: spec.into(),
            }
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
        self.precompiles
            .run(context, address, inputs, is_static, gas_limit)
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.precompiles.warm_addresses()
    }

    fn contains(&self, address: &Address) -> bool {
        self.precompiles.contains(address)
    }
}

/// Custom EVM configuration.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct StrataEvmFactory;

impl EvmFactory for StrataEvmFactory {
    type Evm<DB: reth_evm::Database, I: revm::Inspector<Self::Context<DB>>> =
        EthEvm<DB, I, CustomPrecompiles>;

    type Context<DB: reth_evm::Database> = EthEvmContext<DB>;

    type Tx = TxEnv;
    type Error<DBError: std::error::Error + Send + Sync + 'static> = EVMError<DBError>;

    type HaltReason = HaltReason;

    type Spec = SpecId;

    fn create_evm<DB: reth_evm::Database>(
        &self,
        db: DB,
        input: EvmEnv,
    ) -> Self::Evm<DB, revm::inspector::NoOpInspector> {
        let evm = Context::mainnet()
            .with_db(db)
            .with_cfg(input.cfg_env)
            .with_block(input.block_env)
            .build_mainnet_with_inspector(NoOpInspector {})
            .with_precompiles(CustomPrecompiles::new());

        EthEvm::new(evm, false)
    }

    fn create_evm_with_inspector<DB: reth_evm::Database, I: revm::Inspector<Self::Context<DB>>>(
        &self,
        db: DB,
        input: reth_evm::EvmEnv<Self::Spec>,
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

/// Builds a regular ethereum block executor that uses the custom EVM.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct StrataExecutorBuilder;

impl<Node> ExecutorBuilder<Node> for StrataExecutorBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>>,
{
    type EVM = EthEvmConfig<StrataEvmFactory>;

    type Executor = BasicBlockExecutorProvider<Self::EVM>;

    async fn build_evm(
        self,
        ctx: &BuilderContext<Node>,
    ) -> eyre::Result<(Self::EVM, Self::Executor)> {
        let evm_config =
            EthEvmConfig::new_with_evm_factory(ctx.chain_spec(), StrataEvmFactory::default());
        Ok((
            evm_config.clone(),
            BasicBlockExecutorProvider::new(evm_config),
        ))
    }
}
