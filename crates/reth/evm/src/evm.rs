use std::sync::OnceLock;

use revm::{
    context::{Cfg, ContextTr},
    handler::{EthPrecompiles, PrecompileProvider},
    interpreter::{InputsImpl, InterpreterResult},
    precompile::{PrecompileFn, PrecompileOutput, PrecompileResult, Precompiles},
};
use revm_primitives::{address, hardfork::SpecId, Address, Bytes};

use crate::{constants::SCHNORR_ADDRESS, precompiles::schnorr::verify_schnorr_precompile};

/// A custom precompile that contains static precompiles.
#[derive(Clone, Default)]
pub struct StrataEvmPrecompiles {
    pub precompiles: EthPrecompiles,
}

impl StrataEvmPrecompiles {
    /// Given a [`PrecompileProvider`] and cache for a specific precompiles, create a
    /// wrapper that can be used inside Evm.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Returns precompiles for Fjor spec.
pub fn load_precompiles() -> &'static Precompiles {
    static INSTANCE: OnceLock<Precompiles> = OnceLock::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = Precompiles::berlin().clone();
        // Custom precompile.
        precompiles.extend([(SCHNORR_ADDRESS, verify_schnorr_precompile as PrecompileFn).into()]);
        precompiles
    })
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for StrataEvmPrecompiles {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        self.precompiles = EthPrecompiles {
            precompiles: load_precompiles(),
            spec: spec.into(),
        };
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
