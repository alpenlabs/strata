use std::fmt;

use serde::{de::DeserializeOwned, Serialize};
use sp1_sdk::{
    HashableKey, ProverClient, SP1Proof, SP1ProofWithPublicValues, SP1ProvingKey, SP1PublicValues,
    SP1Stdin, SP1VerifyingKey,
};
use strata_zkvm::{
    Proof, ProofReceipt, ProofType, PublicValues, VerificationKey, ZkVmError, ZkVmHost,
    ZkVmInputBuilder, ZkVmResult,
};

use crate::input::SP1ProofInputBuilder;

/// A host for the `SP1` zkVM that stores the guest program in ELF format.
/// The `SP1Host` is responsible for program execution and proving
#[derive(Clone)]
pub struct SP1Host {
    #[allow(dead_code)]
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

    pub fn init(guest_code: &[u8]) -> Self {
        let client = ProverClient::new();
        let (proving_key, verifying_key) = client.setup(guest_code);
        Self {
            elf: guest_code.to_vec(),
            proving_key,
            verifying_key,
        }
    }
}

impl ZkVmHost for SP1Host {
    type Input<'a> = SP1ProofInputBuilder;
    fn prove<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        proof_type: ProofType,
    ) -> ZkVmResult<ProofReceipt> {
        #[cfg(feature = "mock")]
        {
            std::env::set_var("SP1_PROVER", "mock");
        }

        sp1_sdk::utils::setup_logger();
        let client = ProverClient::new();

        // Start proving
        let mut prover = client.prove(&self.proving_key, prover_input);
        prover = match proof_type {
            ProofType::Compressed => prover.compressed(),
            ProofType::Core => prover.core(),
            ProofType::Groth16 => prover.groth16(),
        };

        let proof_info = prover
            .run()
            .map_err(|e| ZkVmError::ProofGenerationError(e.to_string()))?;

        // Proof serialization
        let proof = Proof::new(bincode::serialize(&proof_info.proof)?);
        let public_values = PublicValues::new(proof_info.public_values.to_vec());

        Ok(ProofReceipt {
            proof,
            public_values,
        })
    }

    fn extract_serde_public_output<T: Serialize + DeserializeOwned>(
        proof: &PublicValues,
    ) -> ZkVmResult<T> {
        let mut proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())?;
        let public_params: T = proof.public_values.read();
        Ok(public_params)
    }

    fn get_verification_key(&self) -> VerificationKey {
        let verification_key = bincode::serialize(&self.verifying_key).unwrap();
        VerificationKey::new(verification_key)
    }

    fn verify(&self, proof: &ProofReceipt) -> ZkVmResult<()> {
        #[cfg(feature = "mock")]
        {
            std::env::set_var("SP1_PROVER", "mock");
        }
        let public_values = SP1PublicValues::from(proof.public_values.as_bytes());
        let proof: SP1Proof = bincode::deserialize(proof.proof.as_bytes())?;
        let sp1_version = sp1_sdk::SP1_CIRCUIT_VERSION.to_string();
        let proof_with_public_values = SP1ProofWithPublicValues {
            proof,
            public_values,
            stdin: SP1Stdin::default(),
            sp1_version,
        };

        let client = ProverClient::new();
        client
            .verify(&proof_with_public_values, &self.verifying_key)
            .map_err(|e| ZkVmError::ProofVerificationError(e.to_string()))?;

        Ok(())
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

    use sp1_sdk::HashableKey;
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
        let proof = zkvm
            .prove(prover_input, ProofType::Core)
            .expect("Failed to generate proof");

        // assert proof verification works
        zkvm.verify(&proof).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out: u32 = SP1Host::extract_serde_public_output(&proof.public_values).expect(
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
        let proof = zkvm
            .prove(prover_input, ProofType::Groth16)
            .expect("Failed to generate proof");

        // Note: For the fixed ELF and fixed SP1 version, the vk is fixed
        assert_eq!(
            zkvm.verifying_key.bytes32(),
            "0x00efb1120491119751e75bc55bc95b64d33f973ecf68fcf5cbff08506c5788f9"
        );

        let filename = "proof-groth16.bin";
        let mut file = File::create(filename).unwrap();
        file.write_all(&bincode::serialize(&proof).expect("bincode serialization failed"))
            .unwrap();
    }
}
