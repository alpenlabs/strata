use std::sync::LazyLock;

use strata_sp1_adapter::SP1Host;
use strata_sp1_guest_builder::{
    GUEST_BTC_BLOCKSPACE_ELF, GUEST_BTC_BLOCKSPACE_PK, GUEST_BTC_BLOCKSPACE_VK,
    GUEST_CHECKPOINT_ELF, GUEST_CHECKPOINT_PK, GUEST_CHECKPOINT_VK, GUEST_CL_AGG_ELF,
    GUEST_CL_AGG_PK, GUEST_CL_AGG_VK, GUEST_CL_STF_ELF, GUEST_CL_STF_PK, GUEST_CL_STF_VK,
    GUEST_EVM_EE_STF_ELF, GUEST_EVM_EE_STF_PK, GUEST_EVM_EE_STF_VK, GUEST_L1_BATCH_ELF,
    GUEST_L1_BATCH_PK, GUEST_L1_BATCH_VK,
};

use crate::primitives::vms::StrataProvingOp;

pub static BTC_BLOCKSPACE_SP1_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
    SP1Host::new_from_bytes(
        &GUEST_BTC_BLOCKSPACE_ELF,
        &GUEST_BTC_BLOCKSPACE_PK,
        &GUEST_BTC_BLOCKSPACE_VK,
    )
});

pub static L1_BATCH_SP1_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
    SP1Host::new_from_bytes(&GUEST_L1_BATCH_ELF, &GUEST_L1_BATCH_PK, &GUEST_L1_BATCH_VK)
});

pub static EVM_EE_STF_SP1_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
    SP1Host::new_from_bytes(
        &GUEST_EVM_EE_STF_ELF,
        &GUEST_EVM_EE_STF_PK,
        &GUEST_EVM_EE_STF_VK,
    )
});

pub static CL_STF_SP1_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
    SP1Host::new_from_bytes(&GUEST_CL_STF_ELF, &GUEST_CL_STF_PK, &GUEST_CL_STF_VK)
});

pub static CL_AGG_SP1_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
    SP1Host::new_from_bytes(&GUEST_CL_AGG_ELF, &GUEST_CL_AGG_PK, &GUEST_CL_AGG_VK)
});

pub static CHECKPOINT_SP1_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
    SP1Host::new_from_bytes(
        &GUEST_CHECKPOINT_ELF,
        &GUEST_CHECKPOINT_PK,
        &GUEST_CHECKPOINT_VK,
    )
});

pub fn get_host(op: &StrataProvingOp) -> &SP1Host {
    match op {
        StrataProvingOp::BtcBlockspace => &*BTC_BLOCKSPACE_SP1_HOST,
        StrataProvingOp::L1Batch => &*L1_BATCH_SP1_HOST,
        StrataProvingOp::EvmEeStf => &*EVM_EE_STF_SP1_HOST,
        StrataProvingOp::ClStf => &*CL_STF_SP1_HOST,
        StrataProvingOp::ClAgg => &*CL_AGG_SP1_HOST,
        StrataProvingOp::Checkpoint => &*CHECKPOINT_SP1_HOST,
    }
}
