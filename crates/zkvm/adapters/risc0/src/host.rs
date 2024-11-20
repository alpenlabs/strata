use std::fmt;

use hex::encode;
use risc0_zkvm::{compute_image_id, default_prover, sha::Digest, ProverOpts, Receipt};
use serde::{de::DeserializeOwned, Serialize};
use strata_zkvm::{
    Proof, ProofType, VerificationKey, ZkVmError, ZkVmHost, ZkVmInputBuilder, ZkVmResult,
};

use crate::input::Risc0ProofInputBuilder;

/// A host for the `Risc0` zkVM that stores the guest program in ELF format
/// The `Risc0Host` is responsible for program execution and proving

#[derive(Clone)]
pub struct Risc0Host {
    elf: Vec<u8>,
    id: Digest,
}

impl Risc0Host {
    pub fn init(guest_code: &[u8]) -> Self {
        let id = compute_image_id(guest_code).expect("invalid elf");
        Risc0Host {
            elf: guest_code.to_vec(),
            id,
        }
    }
}

impl ZkVmHost for Risc0Host {
    type Input<'a> = Risc0ProofInputBuilder<'a>;

    fn prove<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        proof_type: ProofType,
    ) -> ZkVmResult<(Proof, VerificationKey)> {
        #[cfg(feature = "mock")]
        {
            std::env::set_var("RISC0_DEV_MODE", "true");
        }

        // Setup the prover
        let opts = match proof_type {
            ProofType::Core => ProverOpts::default(),
            ProofType::Compressed => ProverOpts::succinct(),
            ProofType::Groth16 => ProverOpts::groth16(),
        };

        // let prover = get_prover_server(&opts)?;
        let prover = default_prover();

        // Setup verification key
        let program_id =
            compute_image_id(&self.elf).map_err(|e| ZkVmError::InvalidELF(e.to_string()))?;
        let verification_key = bincode::serialize(&program_id)?;

        // Generate the session
        // let mut exec = ExecutorImpl::from_elf(prover_input, &self.elf)?;
        // let session = exec.run()?;

        // Generate the proof
        // let ctx = VerifierContext::default();
        // let proof_info = prover.prove_session(&ctx, &session)?;
        let proof_info = prover
            .prove_with_opts(prover_input, &self.elf, &opts)
            .map_err(|e| ZkVmError::ProofGenerationError(e.to_string()))?;

        // Proof serialization
        let serialized_proof = bincode::serialize(&proof_info.receipt)?;
        Ok((
            Proof::new(serialized_proof),
            VerificationKey::new(verification_key),
        ))
    }

    fn extract_raw_public_output(proof: &Proof) -> ZkVmResult<Vec<u8>> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        Ok(receipt.journal.bytes)
    }

    fn extract_serde_public_output<T: Serialize + DeserializeOwned>(
        proof: &Proof,
    ) -> ZkVmResult<T> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        receipt
            .journal
            .decode()
            .map_err(|e| ZkVmError::DeserializationError {
                source: strata_zkvm::DeserializationErrorSource::Serde(e.to_string()),
            })
    }

    fn get_verification_key(&self) -> VerificationKey {
        VerificationKey::new(self.id.as_bytes().to_vec())
    }

    fn verify(&self, proof: &Proof) -> ZkVmResult<()> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        receipt
            .verify(self.id)
            .map_err(|e| ZkVmError::ProofVerificationError(e.to_string()))?;
        Ok(())
    }
}

impl fmt::Display for Risc0Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "risc0_{}", encode(self.id.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use super::*;
    use crate::Risc0ProofInputBuilder;

    // Adding compiled guest code `TEST_ELF` to save the build time
    // use risc0_zkvm::guest::env;
    // fn main() {
    //     let input: u32 = env::read();
    //     env::commit(&input);
    // }
    const TEST_ELF: &[u8] = include_bytes!("../tests/elf/risc0-zkvm-elf");

    #[test]
    fn test_mock_prover() {
        let input: u32 = 1;
        let host = Risc0Host::init(TEST_ELF);

        // prepare input
        let mut zkvm_input_builder = Risc0ProofInputBuilder::new();
        zkvm_input_builder.write_serde(&input).unwrap();
        let zkvm_input = zkvm_input_builder.build().unwrap();

        // assert proof generation works
        let (proof, _) = host
            .prove(zkvm_input, ProofType::Core)
            .expect("Failed to generate proof");

        // assert proof verification works
        host.verify(&proof).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out: u32 = Risc0Host::extract_serde_public_output(&proof)
            .expect("Failed to extract public outputs");
        assert_eq!(input, out)
    }

    #[test]
    fn test_mock_prover_with_public_param() {
        let input: u32 = 1;
        let zkvm = Risc0Host::init(TEST_ELF);

        // prepare input
        let mut zkvm_input_builder = Risc0ProofInputBuilder::new();
        zkvm_input_builder.write_serde(&input).unwrap();
        let zkvm_input = zkvm_input_builder.build().unwrap();

        // assert proof generation works
        let (proof, _) = zkvm
            .prove(zkvm_input, ProofType::Core)
            .expect("Failed to generate proof");

        // assert proof verification works
        zkvm.verify(&proof).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out: u32 = Risc0Host::extract_serde_public_output(&proof).expect(
            "Failed to extract public
outputs",
        );
        assert_eq!(input, out)
    }

    #[test]
    #[ignore]
    fn test_groth16_proof_gen() {
        // Initialize tracing. In order to view logs, run `RUST_LOG=info cargo run`
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
            .init();

        let input: u32 = 1;

        let zkvm = Risc0Host::init(TEST_ELF);

        // prepare input
        let zkvm_input = Risc0ProofInputBuilder::new()
            .write_serde(&input)
            .unwrap()
            .build()
            .unwrap();

        // assert proof generation works
        let (proof, vk) = zkvm
            .prove(zkvm_input, ProofType::Groth16)
            .expect("Failed to generate proof");

        let expected_vk = vec![
            48, 77, 52, 1, 100, 95, 109, 135, 223, 56, 83, 146, 244, 21, 237, 63, 198, 105, 2, 75,
            135, 48, 52, 165, 178, 24, 200, 186, 174, 191, 212, 184,
        ];

        let filename = "proof-groth16.bin";
        let mut file = File::create(filename).unwrap();
        file.write_all(proof.as_bytes()).unwrap();

        assert_eq!(vk.as_bytes(), expected_vk);
    }
}
