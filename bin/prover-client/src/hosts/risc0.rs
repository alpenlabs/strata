use std::sync::LazyLock;

use strata_primitives::proof::ProofId;
use strata_risc0_adapter::Risc0Host;
use strata_risc0_guest_builder::{
    GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_CHECKPOINT_ELF, GUEST_RISC0_CL_AGG_ELF,
    GUEST_RISC0_CL_STF_ELF, GUEST_RISC0_EVM_EE_STF_ELF, GUEST_RISC0_L1_BATCH_ELF,
};

static BTC_BLOCKSPACE_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF));

static L1_BATCH_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_L1_BATCH_ELF));

static EVM_EE_STF_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF));

static CL_STF_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_CL_STF_ELF));

static CL_AGG_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_CL_AGG_ELF));

static CHECKPOINT_HOST: LazyLock<Risc0Host> =
    std::sync::LazyLock::new(|| Risc0Host::init(GUEST_RISC0_CHECKPOINT_ELF));

pub fn get_host(id: &ProofId) -> &'static Risc0Host {
    match id {
        ProofId::BtcBlockspace(_) => &BTC_BLOCKSPACE_HOST,
        ProofId::L1Batch(_, _) => &L1_BATCH_HOST,
        ProofId::EvmEeStf(_) => &EVM_EE_STF_HOST,
        ProofId::ClStf(_) => &CL_STF_HOST,
        ProofId::ClAgg(_, _) => &CL_AGG_HOST,
        ProofId::Checkpoint(_) => &CHECKPOINT_HOST,
    }
}
