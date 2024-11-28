use strata_sp1_adapter::SP1Host;
use strata_sp1_guest_builder::*;

pub fn btc_blockspace() -> SP1Host {
    SP1Host::new_from_bytes(
        &GUEST_BTC_BLOCKSPACE_ELF,
        &GUEST_BTC_BLOCKSPACE_PK,
        &GUEST_BTC_BLOCKSPACE_VK,
    )
}
pub fn l1_batch() -> SP1Host {
    SP1Host::new_from_bytes(&GUEST_L1_BATCH_ELF, &GUEST_L1_BATCH_PK, &GUEST_L1_BATCH_VK)
}

pub fn evm_ee_stf() -> SP1Host {
    SP1Host::new_from_bytes(
        &GUEST_EVM_EE_STF_ELF,
        &GUEST_EVM_EE_STF_PK,
        &GUEST_EVM_EE_STF_VK,
    )
}

pub fn cl_stf() -> SP1Host {
    SP1Host::new_from_bytes(&GUEST_CL_STF_ELF, &GUEST_CL_STF_PK, &GUEST_CL_STF_VK)
}

pub fn cl_agg() -> SP1Host {
    SP1Host::new_from_bytes(&GUEST_CL_AGG_ELF, &GUEST_CL_AGG_PK, &GUEST_CL_AGG_VK)
}

pub fn checkpoint() -> SP1Host {
    SP1Host::new_from_bytes(
        &GUEST_CHECKPOINT_ELF,
        &GUEST_CHECKPOINT_PK,
        &GUEST_CHECKPOINT_VK,
    )
}
