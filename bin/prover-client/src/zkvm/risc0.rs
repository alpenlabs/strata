use strata_risc0_adapter::Risc0Host;
use strata_risc0_guest_builder::{
    GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_CHECKPOINT_ELF, GUEST_RISC0_CL_AGG_ELF,
    GUEST_RISC0_CL_STF_ELF, GUEST_RISC0_EVM_EE_STF_ELF, GUEST_RISC0_L1_BATCH_ELF,
};

pub fn get_host(vm: ProofVm) -> impl ZkVmHost {
    match vm {
        ProofVm::BtcProving => Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF),
        ProofVm::L1Batch => Risc0Host::init(GUEST_RISC0_L1_BATCH_ELF),
        ProofVm::ELProving => Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF),
        ProofVm::CLProving => Risc0Host::init(GUEST_RISC0_CL_STF_ELF),
        ProofVm::CLAggregation => Risc0Host::init(GUEST_RISC0_CL_AGG_ELF),
        ProofVm::Checkpoint => Risc0Host::init(GUEST_RISC0_CHECKPOINT_ELF),
    }
}
