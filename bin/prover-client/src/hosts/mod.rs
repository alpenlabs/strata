use strata_primitives::proof::ProofZkVmHost;
use strata_rpc_types::ProofKey;
use strata_zkvm::{VerificationKey, ZkVmHost};

pub mod native;
#[cfg(feature = "risc0")]
pub mod risc0;
#[cfg(feature = "sp1")]
pub mod sp1;

pub fn get_verification_key(key: &ProofKey) -> VerificationKey {
    match key.host() {
        ProofZkVmHost::SP1 => {
            #[cfg(feature = "sp1")]
            {
                sp1::get_host(key.id()).get_verification_key()
            }
            #[cfg(not(feature = "sp1"))]
            {
                panic!("The `sp1` feature is not enabled. Enable the feature to use SP1 functionality.");
            }
        }
        ProofZkVmHost::Risc0 => {
            #[cfg(feature = "risc0")]
            {
                risc0::get_host(key.id()).get_verification_key()
            }
            #[cfg(not(feature = "risc0"))]
            {
                panic!("The `risc0` feature is not enabled. Enable the feature to use Risc0 functionality.");
            }
        }
        ProofZkVmHost::Native => native::get_host(key.id()).get_verification_key(),
    }
}
