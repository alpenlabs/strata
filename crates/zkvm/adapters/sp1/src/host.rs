use std::fmt;

use serde::{de::DeserializeOwned, Serialize};
use sp1_sdk::{
    network::FulfillmentStrategy, HashableKey, ProverClient, SP1ProvingKey, SP1VerifyingKey,
};
use strata_zkvm::{
    ProofType, PublicValues, VerificationKey, ZkVmError, ZkVmHost, ZkVmInputBuilder, ZkVmResult,
};

use crate::{input::SP1ProofInputBuilder, proof::SP1ProofReceipt};

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
        let client = ProverClient::from_env();
        let (proving_key, verifying_key) = client.setup(guest_code);
        Self {
            elf: guest_code.to_vec(),
            proving_key,
            verifying_key,
        }
    }

    // TODO: consider moving to ZkVkHost trait.
    pub fn get_elf(&self) -> &[u8] {
        &self.elf
    }
}

impl ZkVmHost for SP1Host {
    type Input<'a> = SP1ProofInputBuilder;
    type ZkVmProofReceipt = SP1ProofReceipt;
    fn prove_inner<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
        proof_type: ProofType,
    ) -> ZkVmResult<SP1ProofReceipt> {
        #[cfg(feature = "mock")]
        {
            std::env::set_var("SP1_PROVER", "mock");
        }

        // Prover network
        if std::env::var("SP1_PROVER").unwrap_or_default() == "network" {
            let prover_client = ProverClient::builder().network().build();

            let network_prover_builder = prover_client
                .prove(&self.proving_key, &prover_input)
                .strategy(FulfillmentStrategy::Reserved);

            let network_prover = match proof_type {
                ProofType::Compressed => network_prover_builder.compressed(),
                ProofType::Core => network_prover_builder.core(),
                ProofType::Groth16 => network_prover_builder.groth16(),
            };

            let proof_info = network_prover
                .run()
                .map_err(|e| ZkVmError::ProofGenerationError(e.to_string()))?;

            return Ok(proof_info.into());
        }

        // Local proving
        let client = ProverClient::from_env();
        let mut prover = client.prove(&self.proving_key, &prover_input);
        prover = match proof_type {
            ProofType::Compressed => prover.compressed(),
            ProofType::Core => prover.core(),
            ProofType::Groth16 => prover.groth16(),
        };

        let proof_info = prover
            .run()
            .map_err(|e| ZkVmError::ProofGenerationError(e.to_string()))?;

        Ok(proof_info.into())
    }

    fn extract_serde_public_output<T: Serialize + DeserializeOwned>(
        public_values: &PublicValues,
    ) -> ZkVmResult<T> {
        let public_params: T = bincode::deserialize(public_values.as_bytes())
            .map_err(|e| ZkVmError::OutputExtractionError { source: e.into() })?;
        Ok(public_params)
    }

    fn get_verification_key(&self) -> VerificationKey {
        let verification_key = bincode::serialize(&self.verifying_key).unwrap();
        VerificationKey::new(verification_key)
    }

    fn verify_inner(&self, proof: &SP1ProofReceipt) -> ZkVmResult<()> {
        let client = ProverClient::from_env();
        client
            .verify(proof.as_ref(), &self.verifying_key)
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
// #[cfg(not(debug_assertions))]
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
        let out: u32 = SP1Host::extract_serde_public_output(proof.public_values()).expect(
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
            "0x00bb534e86ded20a0b5608bae1879132a0e1c1ea324eabae095f336899a93e32"
        );

        // Convert the proof to SP1ProofReceipt and extract inner proof data
        let sp1_proof =
            SP1ProofReceipt::try_from(proof).expect("Failed to convert to SP1ProofReceipt");
        let proof_data = sp1_proof.inner();

        let filename = "proof-groth16.bin";
        let mut file = File::create(filename).unwrap();
        file.write_all(&bincode::serialize(&proof_data).expect("bincode serialization failed"))
            .unwrap();
    }
}
