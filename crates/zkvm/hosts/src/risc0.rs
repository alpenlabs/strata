use std::sync::LazyLock;

use zkaleido_risc0_adapter::Risc0Host;
use strata_risc0_guest_builder::{
    GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_CHECKPOINT_ELF, GUEST_RISC0_CL_AGG_ELF,
    GUEST_RISC0_CL_STF_ELF, GUEST_RISC0_EVM_EE_STF_ELF, GUEST_RISC0_L1_BATCH_ELF,
};

use crate::ProofVm;

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

pub fn get_host(vm: ProofVm) -> &'static Risc0Host {
    match vm {
        ProofVm::BtcProving => &BTC_BLOCKSPACE_HOST,
        ProofVm::L1Batch => &L1_BATCH_HOST,
        ProofVm::ELProving => &EVM_EE_STF_HOST,
        ProofVm::CLProving => &CL_STF_HOST,
        ProofVm::CLAggregation => &CL_AGG_HOST,
        ProofVm::Checkpoint => &CHECKPOINT_HOST,
    }
}
