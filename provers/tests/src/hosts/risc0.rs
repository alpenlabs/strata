use strata_risc0_adapter::Risc0Host;
use strata_risc0_guest_builder::{
    GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_CHECKPOINT_ELF, GUEST_RISC0_CL_AGG_ELF,
    GUEST_RISC0_CL_STF_ELF, GUEST_RISC0_EVM_EE_STF_ELF, GUEST_RISC0_L1_BATCH_ELF,
};

pub fn btc_blockspace() -> Risc0Host {
    Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF)
}
pub fn l1_batch() -> Risc0Host {
    Risc0Host::init(GUEST_RISC0_L1_BATCH_ELF)
}

pub fn evm_ee_stf() -> Risc0Host {
    Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF)
}

pub fn cl_stf() -> Risc0Host {
    Risc0Host::init(GUEST_RISC0_CL_STF_ELF)
}

pub fn cl_agg() -> Risc0Host {
    Risc0Host::init(GUEST_RISC0_CL_AGG_ELF)
}

pub fn checkpoint() -> Risc0Host {
    Risc0Host::init(GUEST_RISC0_CHECKPOINT_ELF)
}
