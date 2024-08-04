use anyhow::{anyhow, Ok};

use serde_json::to_vec;
use zkvm::{Proof, ProverOptions, ZKVMHost, ZKVMVerifier};

pub struct SP1Host {
    elf: Vec<u8>,
    _inputs: Vec<u8>,
    prover_options: ProverOptions,
}

impl SP1Host {}

impl ZKVMHost for SP1Host {
    fn init(guest_code: Vec<u8>, prover_options: zkvm::ProverOptions) -> Self {
        SP1Host {
            elf: guest_code,
            _inputs: Vec::new(),
            prover_options,
        }
    }

    fn prove<T: serde::Serialize>(&self, input: T) -> anyhow::Result<Proof> {
        // Proof seralization
        let serialized_proof = todo!();
        Ok(Proof(serialized_proof))
    }
}

pub struct SP1Verifier;

impl ZKVMVerifier for SP1Verifier {
    fn verify(program_id: [u32; 8], proof: &Proof) -> anyhow::Result<()> {
        todo!()
    }

    fn verify_with_public_params<T: serde::de::DeserializeOwned + serde::Serialize>(
        program_id: [u32; 8],
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()> {
        todo!()
    }

    fn extract_public_output<T: serde::de::DeserializeOwned>(proof: &Proof) -> anyhow::Result<T> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use zkvm::ProverOptions;

    use super::*;

    // Adding compiled guest code `TEST_ELF` to save the build time
    // use risc0_zkvm::guest::env;
    // fn main() {
    //     let input: u32 = env::read();
    //     env::commit(&input);
    // }
    const TEST_ELF: &[u8] = include_bytes!("../elf/risc0-zkvm-elf");
    const TEST_ELF_PROGRAM_ID: [u32; 8] = [
        20204848, 2272092004, 2454927583, 1072502260, 1258449350, 2771660935, 3133675698,
        3100950446,
    ];

    #[test]
    fn test_mock_prover() {
        let input: u32 = 1;
        let zkvm = SP1Host::init(TEST_ELF.to_vec(), ProverOptions::default());

        // assert proof generation works
        let proof = zkvm.prove(input).expect("Failed to generate proof");

        // assert proof verification works
        SP1Verifier::verify(TEST_ELF_PROGRAM_ID, &proof).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out: u32 =
            SP1Verifier::extract_public_output(&proof).expect("Failed to extract public outputs");
        assert_eq!(input, out)
    }

    #[test]
    fn test_mock_prover_with_public_param() {
        let input: u32 = 1;
        let zkvm = SP1Host::init(TEST_ELF.to_vec(), ProverOptions::default());

        // assert proof generation works
        let proof = zkvm.prove(input).expect("Failed to generate proof");

        // assert proof verification works
        SP1Verifier::verify_with_public_params(TEST_ELF_PROGRAM_ID, input, &proof)
            .expect("Proof verification failed");
    }
}
