use crate::primitives::vms::ProofVm;

pub mod native;
#[cfg(feature = "risc0")]
pub mod risc0;
#[cfg(feature = "risc0")]
use strata_risc0_adapter::Risc0Host;
#[cfg(feature = "sp1")]
pub mod sp1;
#[cfg(feature = "sp1")]
use strata_sp1_adapter::SP1Host;

#[cfg(all(feature = "risc0", not(feature = "sp1")))]
pub fn get_host(vm: ProofVm) -> Risc0Host {
    risc0::get_host(vm)
}

#[cfg(all(feature = "sp1", not(feature = "risc0")))]
pub fn get_host(vm: ProofVm) -> &'static SP1Host {
    sp1::get_host(vm)
}

// Native Host is used if both risc0 and sp1 are disabled
#[cfg(all(not(feature = "sp1"), not(feature = "risc0")))]
pub fn get_host(vm: ProofVm) -> NativeHost {
    use strata_native_zkvm_adapter::NativeHost;

    native::get_host(vm)
}

// Use SP1 if both risc0 and sp1 are enabled
#[cfg(all(feature = "sp1", feature = "risc0"))]
pub fn get_host(vm: ProofVm) -> &'static SP1Host {
    sp1::get_host(vm)
}
