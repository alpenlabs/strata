use strata_zkvm::ZkVmHost;

use crate::primitives::vms::ProofVm;

pub mod native;
#[cfg(feature = "risc0")]
pub mod risc0;
#[cfg(feature = "sp1")]
pub mod sp1;

#[cfg(all(feature = "risc0", not(feature = "sp1")))]
pub fn get_host(vm: ProofVm) -> impl ZkVmHost {
    risc0::get_host(vm)
}

#[cfg(all(feature = "sp1", not(feature = "risc0")))]
pub fn get_host(vm: ProofVm) -> impl ZkVmHost {
    sp1::get_host(vm)
}

// Native Host is used if both risc0 and sp1 are disabled
#[cfg(all(not(feature = "sp1"), not(feature = "risc0")))]
pub fn get_host(vm: ProofVm) -> impl ZkVmHost {
    native::get_host(vm)
}

// Use SP1 if both risc0 and sp1 are enabled
#[cfg(all(feature = "sp1", feature = "risc0"))]
pub fn get_host(vm: ProofVm) -> impl ZkVmHost {
    sp1::get_host(vm)
}
