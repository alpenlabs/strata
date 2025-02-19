use std::sync::LazyLock;

use strata_sp1_guest_builder::*;
use zkaleido_sp1_adapter::SP1Host;

use crate::ProofVm;

pub static BTC_BLOCKSPACE_HOST: LazyLock<SP1Host> =
    std::sync::LazyLock::new(|| SP1Host::new_from_bytes(&GUEST_BTC_BLOCKSPACE_PK));

pub static EVM_EE_STF_HOST: LazyLock<SP1Host> =
    std::sync::LazyLock::new(|| SP1Host::new_from_bytes(&GUEST_EVM_EE_STF_PK));

pub static CL_STF_HOST: LazyLock<SP1Host> =
    std::sync::LazyLock::new(|| SP1Host::new_from_bytes(&GUEST_CL_STF_PK));

pub static CL_AGG_HOST: LazyLock<SP1Host> =
    std::sync::LazyLock::new(|| SP1Host::new_from_bytes(&GUEST_CL_AGG_PK));

pub static CHECKPOINT_HOST: LazyLock<SP1Host> =
    std::sync::LazyLock::new(|| SP1Host::new_from_bytes(&GUEST_CHECKPOINT_PK));

pub fn get_host(vm: ProofVm) -> &'static SP1Host {
    match vm {
        ProofVm::BtcProving => &BTC_BLOCKSPACE_HOST,
        ProofVm::ELProving => &EVM_EE_STF_HOST,
        ProofVm::CLProving => &CL_STF_HOST,
        ProofVm::CLAggregation => &CL_AGG_HOST,
        ProofVm::Checkpoint => &CHECKPOINT_HOST,
    }
}
