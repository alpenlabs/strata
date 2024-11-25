use std::{fmt, time::Instant};

use anyhow::Ok;
use serde::{de::DeserializeOwned, Serialize};
use sp1_sdk::{
    HashableKey, ProverClient, SP1ProofWithPublicValues, SP1ProvingKey, SP1VerifyingKey,
};
use strata_zkvm::{Proof, ProofInfo, ProofType, VerificationKey, ZkVmHost, ZkVmInputBuilder};

use crate::input::SP1ProofInputBuilder;

/// A host for the `SP1` zkVM that stores the guest program in ELF format.
/// The `SP1Host` is responsible for program execution and proving
#[derive(Clone)]
pub struct SP1Host {
    elf: Vec<u8>,
    proving_key: SP1ProvingKey,
    verifying_key: SP1VerifyingKey,
}

impl SP1Host {
    pub fn new(elf: &[u8], proving_key: SP1ProvingKey, verifying_key: SP1VerifyingKey) -> Self {
        Self {
            elf: elf.to_vec(),
            proving_key,
            verifying_key,
        }
    }

    pub fn new_from_bytes(
        elf: &[u8],
        proving_key_bytes: &[u8],
        verifying_key_bytes: &[u8],
    ) -> Self {
        let proving_key: SP1ProvingKey =
            bincode::deserialize(proving_key_bytes).expect("invalid sp1 pk bytes");
        let verifying_key: SP1VerifyingKey =
            bincode::deserialize(verifying_key_bytes).expect("invalid sp1 vk bytes");
        SP1Host::new(elf, proving_key, verifying_key)
    }
}

impl ZkVmHost for SP1Host {
    type Input<'a> = SP1ProofInputBuilder;
    fn prove<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        proof_type: ProofType,
    ) -> anyhow::Result<(Proof, ProofInfo)> {
        #[cfg(feature = "mock")]
        {
            std::env::set_var("SP1_PROVER", "mock");
        }
        let start = Instant::now();

        sp1_sdk::utils::setup_logger();
        let client = ProverClient::new();

        let (_, report) = client.execute(&self.elf, prover_input.clone()).run()?;

        // Start proving
        let mut prover = client.prove(&self.proving_key, prover_input);
        prover = match proof_type {
            ProofType::Compressed => prover.compressed(),
            ProofType::Core => prover.core(),
            ProofType::Groth16 => prover.groth16(),
        };

        let proof = prover.run()?;

        // Proof serialization
        let serialized_proof = bincode::serialize(&proof)?;
        let proof = Proof::new(serialized_proof);
        let info = ProofInfo::new(
            report.total_instruction_count(),
            start.elapsed().as_micros(),
        );

        Ok((proof, info))
    }

    fn get_verification_key(&self) -> VerificationKey {
        let verification_key = bincode::serialize(&self.verifying_key).unwrap();
        VerificationKey::new(verification_key)
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<()> {
        let proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())?;

        let client = ProverClient::new();
        client.verify(&proof, &self.verifying_key)?;

        Ok(())
    }

    fn extract_public_output<T: Serialize + DeserializeOwned>(proof: &Proof) -> anyhow::Result<T> {
        let mut proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())?;
        let public_params: T = proof.public_values.read();
        Ok(public_params)
    }

    fn extract_raw_public_output(proof: &Proof) -> anyhow::Result<Vec<u8>> {
        let proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())?;
        Ok(proof.public_values.as_slice().to_vec())
    }
}

impl fmt::Display for SP1Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sp1_{}", self.verifying_key.bytes32())
    }
}

// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(test)]
#[cfg(not(debug_assertions))]
mod tests {

    use std::{fs::File, io::Write};

    use sp1_sdk::{HashableKey, SP1VerifyingKey};
    use strata_zkvm::{ProofType, ZkVmHost};

    use super::*;

    // Adding compiled guest code `TEST_ELF` to save the build time
    // #![no_main]
    // sp1_zkvm::entrypoint!(main);
    // fn main() {
    //     let n = sp1_zkvm::io::read::<u32>();
    //     sp1_zkvm::io::commit(&n);
    // }
    const TEST_ELF: &[u8] = include_bytes!("../tests/elf/riscv32im-succinct-zkvm-elf");

    #[test]
    fn test_mock_prover() {
        let input: u32 = 1;

        let mut prover_input_builder = SP1ProofInputBuilder::new();
        prover_input_builder.write_serde(&input).unwrap();
        let prover_input = prover_input_builder.build().unwrap();

        // assert proof generation works
        let zkvm = SP1Host::init(TEST_ELF);
        let (proof, _) = zkvm
            .prove(prover_input, ProofType::Core)
            .expect("Failed to generate proof");

        // assert proof verification works
        zkvm.verify(&proof).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out: u32 = SP1Host::extract_public_output(&proof).expect(
            "Failed to extract public
    outputs",
        );
        assert_eq!(input, out)
    }

    #[test]
    fn test_groth16_proof_generation() {
        sp1_sdk::utils::setup_logger();

        let input: u32 = 1;

        let prover_input = SP1ProofInputBuilder::new()
            .write_serde(&input)
            .unwrap()
            .build()
            .unwrap();

        let zkvm = SP1Host::init(TEST_ELF);

        // assert proof generation works
        let (proof, vk) = zkvm
            .prove(prover_input, ProofType::Groth16)
            .expect("Failed to generate proof");

        let vk: SP1VerifyingKey = bincode::deserialize(vk.as_bytes()).unwrap();

        // Note: For the fixed ELF and fixed SP1 version, the vk is fixed
        assert_eq!(
            vk.bytes32(),
            "0x00efb1120491119751e75bc55bc95b64d33f973ecf68fcf5cbff08506c5788f9"
        );

        let filename = "proof-groth16.bin";
        let mut file = File::create(filename).unwrap();
        file.write_all(proof.as_bytes()).unwrap();
    }
}
