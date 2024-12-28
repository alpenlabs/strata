use std::{fmt, sync::Arc};

use strata_zkvm::{
    Proof, ProofReceipt, ProofType, PublicValues, VerificationKey, ZkVmError, ZkVmHost, ZkVmResult,
};

use crate::{env::NativeMachine, input::NativeMachineInputBuilder, proof::NativeProofReceipt};

type ProcessProofFn = dyn Fn(&NativeMachine) -> ZkVmResult<()> + Send + Sync;

#[derive(Clone)]
pub struct NativeHost {
    pub process_proof: Arc<Box<ProcessProofFn>>,
}

impl ZkVmHost for NativeHost {
    type Input<'a> = NativeMachineInputBuilder;
    type ZkVmProofReceipt = NativeProofReceipt;

    fn prove_inner<'a>(
        &self,
        native_machine: NativeMachine,
        _proof_type: ProofType,
    ) -> ZkVmResult<NativeProofReceipt> {
        (self.process_proof)(&native_machine)?;
        let output = native_machine.state.borrow().output.clone();
        let proof = Proof::new(output.clone());
        let public_values = PublicValues::new(output);
        Ok(ProofReceipt::new(proof, public_values).try_into()?)
    }

    fn get_verification_key(&self) -> VerificationKey {
        VerificationKey::default()
    }

    fn extract_serde_public_output<T: serde::Serialize + serde::de::DeserializeOwned>(
        public_values_raw: &PublicValues,
    ) -> ZkVmResult<T> {
        let public_params: T = bincode::deserialize(public_values_raw.as_bytes())
            .map_err(|e| ZkVmError::OutputExtractionError { source: e.into() })?;
        Ok(public_params)
    }

    fn verify_inner(&self, proof: &NativeProofReceipt) -> ZkVmResult<()> {
        let receipt = proof.clone().inner();
        if receipt.proof().as_bytes() != receipt.public_values().as_bytes() {
            return Err(ZkVmError::ProofVerificationError(
                "Proof does not match public values".to_string(),
            ));
        }

        Ok(())
    }
}

impl fmt::Display for NativeHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native")
    }
}
