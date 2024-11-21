use std::{fmt, time::Instant};

use hex::encode;
use risc0_zkvm::{compute_image_id, default_prover, sha::Digest, ProverOpts, Receipt};
use serde::de::DeserializeOwned;
use strata_zkvm::{Proof, ProofInfo, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder};

use crate::input::Risc0ProofInputBuilder;

/// A host for the `RiscZero` zkVM that stores the guest program in ELF format
/// The `Risc0Host` is responsible for program execution and proving

#[derive(Clone)]
pub struct Risc0Host {
    elf: Vec<u8>,
    id: Digest,
}

impl Risc0Host {
    pub fn init(guest_code: &[u8]) -> Self {
        let id = compute_image_id(guest_code).expect("invalid elf");
        Self {
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
    ) -> anyhow::Result<(Proof, ProofInfo)> {
        let start = Instant::now();

        // Setup the prover
        let opts = match proof_type {
            ProofType::Core => ProverOpts::default(),
            ProofType::Compressed => ProverOpts::succinct(),
            ProofType::Groth16 => ProverOpts::groth16(),
        };

        // let prover = get_prover_server(&opts)?;
        let prover = default_prover();

        // Generate the session
        // let mut exec = ExecutorImpl::from_elf(prover_input, &self.elf)?;
        // let session = exec.run()?;

        // Generate the proof
        // let ctx = VerifierContext::default();
        // let proof_info = prover.prove_session(&ctx, &session)?;
        let proof_info = prover.prove_with_opts(prover_input, &self.elf, &opts)?;

        // Proof serialization
        let serialized_proof = bincode::serialize(&proof_info.receipt)?;

        let proof = Proof::new(serialized_proof);
        let info = ProofInfo::new(proof_info.stats.total_cycles, start.elapsed());

        Ok((proof, info))
    }

    fn get_verification_key(&self) -> VerificationKey {
        VerificationKey(self.id.as_bytes().to_vec())
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        // TODO: maybe cache this?
        let vk = compute_image_id(&self.elf)?;
        receipt.verify(vk)?;
        Ok(())
    }
    fn extract_public_output<T: DeserializeOwned>(proof: &Proof) -> anyhow::Result<T> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        Ok(receipt.journal.decode()?)
    }
    fn extract_raw_public_output(proof: &Proof) -> anyhow::Result<Vec<u8>> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        Ok(receipt.journal.bytes)
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
        let out: u32 = Risc0Host::extract_public_output(&proof).expect(
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
        let (proof, _) = zkvm
            .prove(zkvm_input, ProofType::Groth16)
            .expect("Failed to generate proof");

        let expected_vk = vec![
            48, 77, 52, 1, 100, 95, 109, 135, 223, 56, 83, 146, 244, 21, 237, 63, 198, 105, 2, 75,
            135, 48, 52, 165, 178, 24, 200, 186, 174, 191, 212, 184,
        ];

        let filename = "proof-groth16.bin";
        let mut file = File::create(filename).unwrap();
        file.write_all(proof.as_bytes()).unwrap();

        assert_eq!(zkvm.get_verification_key().as_bytes(), expected_vk);
    }
}
