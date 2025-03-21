use std::sync::LazyLock;

use strata_primitives::proof::ProofContext;
use strata_risc0_guest_builder::{
    GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_CHECKPOINT_ELF, GUEST_RISC0_CL_STF_ELF,
    GUEST_RISC0_EVM_EE_STF_ELF,
};
use zkaleido_risc0_host::Risc0Host;

static BTC_BLOCKSPACE_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF));

static EVM_EE_STF_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF));

static CL_STF_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_CL_STF_ELF));

static CHECKPOINT_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_CHECKPOINT_ELF));

/// Returns a reference to the appropriate [`Risc0Host`] instance based on the given
/// [`ProofContext`].
///
/// This function maps the [`ProofContext`] variant to its corresponding static [`Risc0Host`]
/// instance, allowing for efficient host selection for different proof types.
pub fn get_host(id: &ProofContext) -> &'static Risc0Host {
    match id {
        ProofContext::BtcBlockspace(..) => &BTC_BLOCKSPACE_HOST,
        ProofContext::EvmEeStf(..) => &EVM_EE_STF_HOST,
        ProofContext::ClStf(..) => &CL_STF_HOST,
        ProofContext::Checkpoint(..) => &CHECKPOINT_HOST,
    }
}
