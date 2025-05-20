use alloy_evm::{eth::EthEvmContext, Database, Evm};
use revm::{
    context::{
        result::{EVMError, HaltReason},
        ContextSetters, ContextTr, Evm as RevmEvm, EvmData, TxEnv,
    },
    handler::{
        instructions::{EthInstructions, InstructionProvider},
        EthPrecompiles, EvmTr, PrecompileProvider,
    },
    inspector::{inspect_instructions, InspectorEvmTr, JournalExt, NoOpInspector},
    interpreter::{
        interpreter::EthInterpreter, InputsImpl, Interpreter, InterpreterResult, InterpreterTypes,
    },
    Context, Inspector, MainBuilder, MainContext,
};
use revm_primitives::hardfork::SpecId;

/// MyEvm variant of the EVM.
pub struct MyEvm<CTX, INSP, PRECOMPILE = EthPrecompiles> {
    pub inner: RevmEvm<CTX, INSP, EthInstructions<EthInterpreter, CTX>, PRECOMPILE>,
    pub inspect: bool,
}

impl<CTX: ContextTr, INSP> MyEvm<CTX, INSP> {
    pub fn new(ctx: CTX, inspector: INSP) -> Self {
        let evm = RevmEvm {
            data: EvmData { ctx, inspector },
            instruction: EthInstructions::new_mainnet(),
            precompiles: EthPrecompiles::default(),
        };

        Self {
            inner: evm,
            inspect: false,
        }
    }
}

impl<CTX: ContextTr, INSP> EvmTr for MyEvm<CTX, INSP>
where
    CTX: ContextTr,
{
    type Context = CTX;
    type Instructions = EthInstructions<EthInterpreter, CTX>;
    type Precompiles = EthPrecompiles;

    fn ctx(&mut self) -> &mut Self::Context {
        &mut self.inner.data.ctx
    }

    fn ctx_ref(&self) -> &Self::Context {
        self.inner.ctx_ref()
    }

    fn ctx_instructions(&mut self) -> (&mut Self::Context, &mut Self::Instructions) {
        self.inner.ctx_instructions()
    }

    fn run_interpreter(
        &mut self,
        interpreter: &mut Interpreter<
            <Self::Instructions as InstructionProvider>::InterpreterTypes,
        >,
    ) -> <<Self::Instructions as InstructionProvider>::InterpreterTypes as InterpreterTypes>::Output
    {
        self.inner.run_interpreter(interpreter)
    }

    fn ctx_precompiles(&mut self) -> (&mut Self::Context, &mut Self::Precompiles) {
        self.inner.ctx_precompiles()
    }
}

impl<CTX: ContextTr, INSP> InspectorEvmTr for MyEvm<CTX, INSP>
where
    CTX: ContextSetters<Journal: JournalExt>,
    INSP: Inspector<CTX, EthInterpreter>,
{
    type Inspector = INSP;

    fn inspector(&mut self) -> &mut Self::Inspector {
        self.inner.inspector()
    }

    fn ctx_inspector(&mut self) -> (&mut Self::Context, &mut Self::Inspector) {
        self.inner.ctx_inspector()
    }

    fn run_inspect_interpreter(
        &mut self,
        interpreter: &mut Interpreter<
            <Self::Instructions as InstructionProvider>::InterpreterTypes,
        >,
    ) -> <<Self::Instructions as InstructionProvider>::InterpreterTypes as InterpreterTypes>::Output
    {
        let context = &mut self.inner.data.ctx;
        let instructions = &mut self.inner.instruction;
        let inspector = &mut self.inner.data.inspector;

        inspect_instructions(
            context,
            interpreter,
            inspector,
            instructions.instruction_table(),
        )
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

    fn block(&self) -> &revm::context::BlockEnv {
        todo!()
    }

    fn transact_raw(
        &mut self,
        tx: Self::Tx,
    ) -> Result<revm::context::result::ResultAndState<Self::HaltReason>, Self::Error> {
        todo!()
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
        todo!()
    }

    fn finish(self) -> (Self::DB, alloy_evm::EvmEnv<Self::Spec>)
    where
        Self: Sized,
    {
        todo!()
    }

    fn set_inspector_enabled(&mut self, enabled: bool) {
        todo!()
    }
}
