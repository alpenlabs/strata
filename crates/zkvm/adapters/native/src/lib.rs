mod env;
mod host;
mod input;
mod proof;

pub use env::NativeMachine;
pub use host::NativeHost;
use strata_zkvm::{Proof, ZkVmError, ZkVmResult};

pub fn verify_groth16(
    proof: &Proof,
    _vkey_hash: &[u8; 32],
    committed_values_raw: &[u8],
) -> ZkVmResult<()> {
    if proof.as_bytes() != committed_values_raw {
        return Err(ZkVmError::ProofVerificationError(
            "Proof does not match public values".to_string(),
        ));
    }

    Ok(())
}
