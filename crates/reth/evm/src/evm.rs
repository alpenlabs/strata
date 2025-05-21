use std::ops::{Deref, DerefMut};

use alloy_evm::{eth::EthEvmContext, Database, Evm, EvmEnv};
use revm::{
    context::{
        result::{EVMError, HaltReason, ResultAndState},
        BlockEnv, ContextSetters, ContextTr, Evm as RevmEvm, EvmData, TxEnv,
    },
    handler::{
        instructions::{EthInstructions, InstructionProvider},
        EthPrecompiles, EvmTr, PrecompileProvider,
    },
    inspector::{inspect_instructions, InspectorEvmTr, JournalExt, NoOpInspector},
    interpreter::{
        interpreter::EthInterpreter, InputsImpl, Interpreter, InterpreterResult, InterpreterTypes,
    },
    precompile, Context, ExecuteEvm, InspectEvm, Inspector, MainBuilder, MainContext,
};
use revm_primitives::hardfork::SpecId;

use crate::strata_evm::StrataEvm;

pub struct MyEvm<DB: Database, I, PRECOMPILE = EthPrecompiles> {
    inner: StrataEvm<
        EthEvmContext<DB>,
        I,
        EthInstructions<EthInterpreter, EthEvmContext<DB>>,
        PRECOMPILE,
    >,
    inspect: bool,
}

impl<DB: Database, I, PRECOMPILE> MyEvm<DB, I, PRECOMPILE> {
    /// Creates a new Ethereum EVM instance.
    ///
    /// The `inspect` argument determines whether the configured [`Inspector`] of the given
    /// [`RevmEvm`] should be invoked on [`Evm::transact`].
    pub const fn new(
        evm: StrataEvm<
            EthEvmContext<DB>,
            I,
            EthInstructions<EthInterpreter, EthEvmContext<DB>>,
            PRECOMPILE,
        >,
        inspect: bool,
    ) -> Self {
        Self {
            inner: evm,
            inspect,
        }
    }

    pub fn another_new(
        ctx: EthEvmContext<DB>,
        instruction: EthInstructions<EthInterpreter, EthEvmContext<DB>>,
        precompiles: PRECOMPILE,
    ) {
        // let evm: StrataEvm<
        //     EthEvmContext<DB>,
        //     I,
        //     EthInstructions<EthInterpreter, EthEvmContext<DB>>,
        //     PRECOMPILE,
        // > = StrataEvm::new(ctx, instruction, precompiles);
        // let evm = Self {
        //     inner: evm,
        //     inspect: false,
        // };
        todo!()
    }

    /// Consumes self and return the inner EVM instance.
    pub fn into_inner(
        self,
    ) -> StrataEvm<
        EthEvmContext<DB>,
        I,
        EthInstructions<EthInterpreter, EthEvmContext<DB>>,
        PRECOMPILE,
    > {
        self.inner
    }

    /// Provides a reference to the EVM context.
    pub const fn ctx(&self) -> &EthEvmContext<DB> {
        &self.inner.data.ctx
    }

    /// Provides a mutable reference to the EVM context.
    pub fn ctx_mut(&mut self) -> &mut EthEvmContext<DB> {
        &mut self.inner.data.ctx
    }

    /// Provides a mutable reference to the EVM inspector.
    pub fn inspector_mut(&mut self) -> &mut I {
        &mut self.inner.data.inspector
    }
}

impl<DB: Database, I, PRECOMPILE> Deref for MyEvm<DB, I, PRECOMPILE> {
    type Target = EthEvmContext<DB>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.ctx()
    }
}

impl<DB: Database, I, PRECOMPILE> DerefMut for MyEvm<DB, I, PRECOMPILE> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ctx_mut()
    }
}

impl<DB, I, PRECOMPILE> Evm for MyEvm<DB, I, PRECOMPILE>
where
    DB: Database,
    I: Inspector<EthEvmContext<DB>>,
    PRECOMPILE: PrecompileProvider<EthEvmContext<DB>, Output = InterpreterResult>,
{
    type DB = DB;

    type Tx = TxEnv;

    type Error = EVMError<DB::Error>;

    type HaltReason = HaltReason;

    type Spec = SpecId;

    fn block(&self) -> &BlockEnv {
        &self.block
    }

    fn transact_raw(&mut self, tx: Self::Tx) -> Result<ResultAndState, Self::Error> {
        todo!()
        // if self.inspect {
        //     self.inner.set_tx(tx);
        //     self.inner.inspect_replay()
        // } else {
        //     self.inner.transact(tx)
        // }
    }

    fn transact_system_call(
        &mut self,
        caller: revm_primitives::Address,
        contract: revm_primitives::Address,
        data: revm_primitives::Bytes,
    ) -> Result<revm::context::result::ResultAndState<Self::HaltReason>, Self::Error> {
        todo!()
    }

    fn db_mut(&mut self) -> &mut Self::DB {
        &mut self.journaled_state.database
    }

    fn finish(self) -> (Self::DB, EvmEnv<Self::Spec>) {
        let Context {
            block: block_env,
            cfg: cfg_env,
            journaled_state,
            ..
        } = self.inner.data.ctx;

        (journaled_state.database, EvmEnv { block_env, cfg_env })
    }

    fn set_inspector_enabled(&mut self, enabled: bool) {
        self.inspect = enabled;
    }
}
