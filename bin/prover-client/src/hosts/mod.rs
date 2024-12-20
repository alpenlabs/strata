use strata_primitives::proof::ProofZkVm;
use strata_rpc_types::ProofKey;
use strata_zkvm::{VerificationKey, ZkVmHost};

pub mod native;
#[cfg(feature = "risc0")]
pub mod risc0;
#[cfg(feature = "sp1")]
pub mod sp1;

/// Retrieves the [`VerificationKey`] for the specified proof key.
///
/// This function determines the appropriate ZkVm host based on the provided [`ProofKey`]
/// and retrieves the corresponding verification key.
///
/// # Panics
///
/// * If the `sp1` feature is not enabled and an SP1 host is requested.
/// * If the `risc0` feature is not enabled and a Risc0 host is requested.
/// * If the host type is unsupported or not recognized.
pub fn get_verification_key(key: &ProofKey) -> VerificationKey {
    match key.host() {
        ProofZkVm::SP1 => {
            #[cfg(feature = "sp1")]
            {
                sp1::get_host(key.context()).get_verification_key()
            }
            #[cfg(not(feature = "sp1"))]
            {
                panic!("The `sp1` feature is not enabled. Enable the feature to use SP1 functionality.");
            }
        }
        ProofZkVm::Risc0 => {
            #[cfg(feature = "risc0")]
            {
                risc0::get_host(key.context()).get_verification_key()
            }
            #[cfg(not(feature = "risc0"))]
            {
                panic!("The `risc0` feature is not enabled. Enable the feature to use Risc0 functionality.");
            }
        }
        ProofZkVm::Native => native::get_host(key.context()).get_verification_key(),
        _ => panic!("Unsupported ZkVm"),
    }
}

pub enum ZkVmHostInstance {
    Native(strata_native_zkvm_adapter::NativeHost),

    #[cfg(feature = "sp1")]
    SP1(&'static strata_sp1_adapter::SP1Host),

    #[cfg(feature = "risc0")]
    Risc0(&'static strata_risc0_adapter::Risc0Host),
}

pub fn resolve_host(proof_key: &ProofKey) -> ZkVmHostInstance {
    match proof_key.host() {
        ProofZkVm::Native => ZkVmHostInstance::Native(native::get_host(proof_key.context())),
        ProofZkVm::SP1 => {
            #[cfg(feature = "sp1")]
            {
                ZkVmHostInstance::SP1(sp1::get_host(proof_key.context()))
            }
            #[cfg(not(feature = "sp1"))]
            {
                panic!(
                    "The `sp1` feature is not enabled. Enable the feature to use SP1 functionality"
                );
            }
        }
        ProofZkVm::Risc0 => {
            #[cfg(feature = "risc0")]
            {
                ZkVmHostInstance::Risc0(risc0::get_host(proof_key.context()))
            }
            #[cfg(not(feature = "risc0"))]
            {
                panic!("The `risc0` feature is not enabled. Enable the feature to use Risc0 functionality");
            }
        }
        _ => panic!("Unsupported ZkVm"),
    }
}
