use std::{fmt, sync::Arc};

use strata_zkvm::{Proof, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder, ZkVmResult};

use crate::{input::NativeMachineInputBuilder, zkvm::NativeMachine};

#[derive(Debug)]
pub struct NativeHost<F>
where
    F: Fn(&NativeMachine) -> ZkVmResult<()> + Send + Sync + 'static,
{
    pub process_proof: Arc<F>,
}

impl<F> Clone for NativeHost<F>
where
    F: Fn(&NativeMachine) -> ZkVmResult<()> + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            process_proof: Arc::clone(&self.process_proof),
        }
    }
}

impl<F> ZkVmHost for NativeHost<F>
where
    F: Fn(&NativeMachine) -> ZkVmResult<()> + Send + Sync + 'static,
{
    type Input<'a> = NativeMachineInputBuilder;

    fn prove<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        _proof_type: ProofType,
    ) -> ZkVmResult<(Proof, VerificationKey)> {
        (self.process_proof)(&prover_input)?;
        let output_ref = prover_input.output.borrow();

        let output = if output_ref.is_empty() {
            vec![]
        } else {
            output_ref[0].clone()
        };
        Ok((Proof::new(output), VerificationKey::new(vec![])))
    }

    fn get_verification_key(&self) -> VerificationKey {
        VerificationKey::new(vec![])
    }

    fn extract_borsh_public_output<T: borsh::BorshDeserialize>(proof: &Proof) -> ZkVmResult<T> {
        Ok(borsh::from_slice(proof.as_bytes()).expect("ser"))
    }

    fn extract_serde_public_output<T: serde::Serialize + serde::de::DeserializeOwned>(
        proof: &Proof,
    ) -> ZkVmResult<T> {
        Ok(bincode::deserialize(proof.as_bytes()).expect("ser"))
    }

    fn extract_raw_public_output(proof: &Proof) -> ZkVmResult<Vec<u8>> {
        Ok(proof.as_bytes().to_vec())
    }

    fn verify(&self, _proof: &Proof) -> ZkVmResult<()> {
        Ok(())
    }
}
impl<F> fmt::Display for NativeHost<F>
where
    F: Fn(&NativeMachine) -> ZkVmResult<()> + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native")
    }
}
