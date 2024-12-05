use strata_sp1_adapter::SP1Host;
use strata_sp1_guest_builder::*;
use strata_zkvm::ZkVmHost;

use crate::primitives::vms::ProofVm;

pub fn get_host(vm: ProofVm) -> impl ZkVmHost {
    match vm {
        ProofVm::BtcProving => SP1Host::new_from_bytes(
            &GUEST_BTC_BLOCKSPACE_ELF,
            &GUEST_BTC_BLOCKSPACE_PK,
            &GUEST_BTC_BLOCKSPACE_VK,
        ),
        ProofVm::L1Batch => SP1Host::new_from_bytes(
            &GUEST_BTC_BLOCKSPACE_ELF,
            &GUEST_BTC_BLOCKSPACE_PK,
            &GUEST_BTC_BLOCKSPACE_VK,
        ),
        ProofVm::ELProving => SP1Host::new_from_bytes(
            &GUEST_EVM_EE_STF_ELF,
            &GUEST_EVM_EE_STF_PK,
            &GUEST_EVM_EE_STF_VK,
        ),
        ProofVm::CLProving => {
            SP1Host::new_from_bytes(&GUEST_CL_STF_ELF, &GUEST_CL_STF_PK, &GUEST_CL_STF_VK)
        }
        ProofVm::CLAggregation => {
            SP1Host::new_from_bytes(&GUEST_CL_AGG_ELF, &GUEST_CL_AGG_PK, &GUEST_CL_AGG_VK)
        }
        ProofVm::Checkpoint => SP1Host::new_from_bytes(
            &GUEST_CHECKPOINT_ELF,
            &GUEST_CHECKPOINT_PK,
            &GUEST_CHECKPOINT_VK,
        ),
    }
}
