use std::sync::LazyLock;

<<<<<<< HEAD
=======
use strata_primitives::proof::ProofContext;
>>>>>>> 54eac344 (fixes)
use strata_sp1_adapter::SP1Host;
use strata_sp1_guest_builder::*;

use crate::ProofVm;

pub static BTC_BLOCKSPACE_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    {
        SP1Host::new_from_bytes(
            &GUEST_BTC_BLOCKSPACE_ELF,
            &GUEST_BTC_BLOCKSPACE_PK,
            &GUEST_BTC_BLOCKSPACE_VK,
        )
    }
});

pub static L1_BATCH_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    {
        SP1Host::new_from_bytes(&GUEST_L1_BATCH_ELF, &GUEST_L1_BATCH_PK, &GUEST_L1_BATCH_VK)
    }
});

pub static EVM_EE_STF_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    {
        SP1Host::new_from_bytes(
            &GUEST_EVM_EE_STF_ELF,
            &GUEST_EVM_EE_STF_PK,
            &GUEST_EVM_EE_STF_VK,
        )
    }
});

pub static CL_STF_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    {
        SP1Host::new_from_bytes(&GUEST_CL_STF_ELF, &GUEST_CL_STF_PK, &GUEST_CL_STF_VK)
    }
});

pub static CL_AGG_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    {
        SP1Host::new_from_bytes(&GUEST_CL_AGG_ELF, &GUEST_CL_AGG_PK, &GUEST_CL_AGG_VK)
    }
});

pub static CHECKPOINT_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    {
        SP1Host::new_from_bytes(
            &GUEST_CHECKPOINT_ELF,
            &GUEST_CHECKPOINT_PK,
            &GUEST_CHECKPOINT_VK,
        )
    }
});

<<<<<<< HEAD
pub fn get_host(vm: ProofVm) -> &'static SP1Host {
    match vm {
        ProofVm::BtcProving => &BTC_BLOCKSPACE_HOST,
        ProofVm::L1Batch => &L1_BATCH_HOST,
        ProofVm::ELProving => &EVM_EE_STF_HOST,
        ProofVm::CLProving => &CL_STF_HOST,
        ProofVm::CLAggregation => &CL_AGG_HOST,
        ProofVm::Checkpoint => &CHECKPOINT_HOST,
=======
pub fn get_host(id: &ProofContext) -> &'static SP1Host {
    match id {
        ProofContext::BtcBlockspace(_) => &BTC_BLOCKSPACE_HOST,
        ProofContext::L1Batch(_, _) => &L1_BATCH_HOST,
        ProofContext::EvmEeStf(_) => &EVM_EE_STF_HOST,
        ProofContext::ClStf(_) => &CL_STF_HOST,
        ProofContext::ClAgg(_, _) => &CL_AGG_HOST,
        ProofContext::Checkpoint(_) => &CHECKPOINT_HOST,
>>>>>>> 54eac344 (fixes)
    }
}