use std::{fmt, sync::Arc};

use strata_zkvm::{Proof, ProofType, VerificationKey, ZkVmHost, ZkVmResult};

use crate::{input::NativeMachineInputBuilder, zkvm::NativeMachine};

type ProcessProofFn = dyn Fn(&NativeMachine) -> ZkVmResult<()> + Send + Sync;

#[derive(Clone)]
pub struct NativeHost {
    pub process_proof: Arc<Box<ProcessProofFn>>,
}

impl ZkVmHost for NativeHost {
    type Input<'a> = NativeMachineInputBuilder;

    fn prove<'a>(
        &self,
        native_machine: NativeMachine,
        _proof_type: ProofType,
    ) -> ZkVmResult<(Proof, VerificationKey)> {
        (self.process_proof)(&native_machine)?;
        let output = native_machine.output.borrow().clone();
        Ok((Proof::new(output), self.get_verification_key()))
    }

    fn get_verification_key(&self) -> VerificationKey {
        VerificationKey::default()
    }

    fn extract_borsh_public_output<T: borsh::BorshDeserialize>(proof: &Proof) -> ZkVmResult<T> {
        borsh::from_slice(proof.as_bytes()).map_err(|e| e.into())
    }

    fn extract_serde_public_output<T: serde::Serialize + serde::de::DeserializeOwned>(
        proof: &Proof,
    ) -> ZkVmResult<T> {
        bincode::deserialize(proof.as_bytes()).map_err(|e| e.into())
    }

    fn extract_raw_public_output(proof: &Proof) -> ZkVmResult<Vec<u8>> {
        Ok(proof.as_bytes().to_vec())
    }

    fn verify(&self, _proof: &Proof) -> ZkVmResult<()> {
        Ok(())
    }
}

impl fmt::Display for NativeHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native")
    }
}
