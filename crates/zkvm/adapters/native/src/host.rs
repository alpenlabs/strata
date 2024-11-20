use std::{fmt, sync::Arc, time::Instant};

use strata_zkvm::{
    Proof, ProofInfo, ProofType, ProofWithInfo, VerificationKey, ZkVmHost, ZkVmInputBuilder,
};

use crate::{input::NativeMachineInputBuilder, zkvm::NativeMachine};

type ProcessProofFn = dyn Fn(&NativeMachine) -> anyhow::Result<()> + Send + Sync;

#[derive(Clone)]
pub struct NativeHost {
    pub process_proof: Arc<Box<ProcessProofFn>>,
}

impl ZkVmHost for NativeHost {
    type Input<'a> = NativeMachineInputBuilder;

    fn prove<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        _proof_type: ProofType,
    ) -> anyhow::Result<ProofWithInfo> {
        let start = Instant::now();

        (self.process_proof)(&prover_input)?;
        let output_ref = prover_input.output.borrow();

        let output = if output_ref.is_empty() {
            vec![]
        } else {
            output_ref[0].clone()
        };

        let proof = Proof::new(output);
        let info = ProofInfo::new(0, start.elapsed());

        Ok(ProofWithInfo::new(proof, info))
    }

    fn get_verification_key(&self) -> VerificationKey {
        VerificationKey::new(vec![])
    }

    fn extract_borsh_public_output<T: borsh::BorshDeserialize>(proof: &Proof) -> anyhow::Result<T> {
        Ok(borsh::from_slice(proof.as_bytes()).expect("ser"))
    }

    fn extract_public_output<T: serde::Serialize + serde::de::DeserializeOwned>(
        proof: &Proof,
    ) -> anyhow::Result<T> {
        Ok(bincode::deserialize(proof.as_bytes()).expect("ser"))
    }

    fn extract_raw_public_output(proof: &Proof) -> anyhow::Result<Vec<u8>> {
        Ok(proof.as_bytes().to_vec())
    }

    fn verify(&self, _proof: &Proof) -> anyhow::Result<()> {
        Ok(())
    }
}
impl fmt::Display for NativeHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native")
    }
}
