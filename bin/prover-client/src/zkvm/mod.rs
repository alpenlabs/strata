use crate::primitives::vms::ProofVm;

pub mod sp1;
use strata_sp1_adapter::SP1Host;

pub fn get_host(vm: ProofVm) -> &'static SP1Host {
    sp1::get_host(vm)
}
