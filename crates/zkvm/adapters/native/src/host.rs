use std::{fmt, sync::Arc};

use strata_zkvm::{
    Proof, ProofReceipt, ProofType, PublicValues, VerificationKey, ZkVmHost, ZkVmResult,
};

use crate::{env::NativeMachine, input::NativeMachineInputBuilder};

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
    ) -> ZkVmResult<ProofReceipt> {
        (self.process_proof)(&native_machine)?;
        let output = native_machine.state.borrow().output.clone();
        let proof = Proof::default();
        let public_values = PublicValues::new(output);
        Ok(ProofReceipt::new(proof, public_values))
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

    fn verify(&self, _proof: &ProofReceipt) -> ZkVmResult<()> {
        Ok(())
    }
}

impl fmt::Display for NativeHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native")
    }
}
