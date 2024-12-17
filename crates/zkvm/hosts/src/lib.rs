// TODO figure out the cfg-if.

#[cfg(feature = "native")]
pub mod native;
#[cfg(feature = "native")]
use strata_native_zkvm_adapter::NativeHost;
#[cfg(feature = "native")]
pub fn get_native_host(vm: ProofVm) -> &'static NativeHost {
    native::get_host(vm)
}

#[cfg(feature = "risc0")]
pub mod risc0;
#[cfg(feature = "risc0")]
use strata_risc0_adapter::Risc0Host;
#[cfg(feature = "risc0")]
pub fn get_risc0_host(vm: ProofVm) -> &'static Risc0Host {
    risc0::get_host(vm)
}

#[cfg(feature = "sp1")]
pub mod sp1;
#[cfg(feature = "sp1")]
use strata_sp1_adapter::SP1Host;
#[cfg(feature = "sp1")]
pub fn get_sp1_host(vm: ProofVm) -> &'static SP1Host {
    sp1::get_host(vm)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofVm {
    BtcProving,
    ELProving,
    CLProving,
    CLAggregation,
    L1Batch,
    Checkpoint,
}
