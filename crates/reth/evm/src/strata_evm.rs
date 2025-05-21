use core::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use revm::context::EvmData;

#[derive(Debug)]
pub struct StrataEvm<CTX, INSP, I, P> {
    pub data: EvmData<CTX, INSP>,
    pub instruction: I,
    pub precompiles: P,
}

impl<CTX, I, P> StrataEvm<CTX, (), I, P> {
    pub fn new(ctx: CTX, instruction: I, precompiles: P) -> StrataEvm<CTX, (), I, P> {
        StrataEvm {
            data: EvmData { ctx, inspector: () },
            instruction,
            precompiles,
        }
    }
}

impl<CTX, I, INSP, P> StrataEvm<CTX, INSP, I, P> {
    pub fn new_with_inspector(ctx: CTX, inspector: INSP, instruction: I, precompiles: P) -> Self {
        StrataEvm {
            data: EvmData { ctx, inspector },
            instruction,
            precompiles,
        }
    }
}

impl<CTX, INSP, I, P> StrataEvm<CTX, INSP, I, P> {
    /// Consumed self and returns new Evm type with given Inspector.
    pub fn with_inspector<OINSP>(self, inspector: OINSP) -> StrataEvm<CTX, OINSP, I, P> {
        StrataEvm {
            data: EvmData {
                ctx: self.data.ctx,
                inspector,
            },
            instruction: self.instruction,
            precompiles: self.precompiles,
        }
    }

    /// Consumes self and returns new Evm type with given Precompiles.
    pub fn with_precompiles<OP>(self, precompiles: OP) -> StrataEvm<CTX, INSP, I, OP> {
        StrataEvm {
            data: self.data,
            instruction: self.instruction,
            precompiles,
        }
    }

    /// Consumes self and returns inner Inspector.
    pub fn into_inspector(self) -> INSP {
        self.data.inspector
    }
}

impl<CTX, INSP, I, P> Deref for StrataEvm<CTX, INSP, I, P> {
    type Target = CTX;

    fn deref(&self) -> &Self::Target {
        &self.data.ctx
    }
}

impl<CTX, INSP, I, P> DerefMut for StrataEvm<CTX, INSP, I, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data.ctx
    }
}
