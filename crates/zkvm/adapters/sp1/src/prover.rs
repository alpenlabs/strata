use std::sync::Arc;

use anyhow::Ok;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::de::DeserializeOwned;
use sp1_sdk::{
    block_on, proto::network::ProofMode, provers::ProverType, HashableKey, NetworkProver,
    ProverClient, SP1ProofWithPublicValues, SP1ProvingKey, SP1VerifyingKey,
};
use strata_zkvm::{
    Proof, ProofWithMetadata, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
};

use crate::{input::SP1ProofInputBuilder, utils::get_proving_keys};

/// A host for the `SP1` zkVM that stores the guest program in ELF format.
/// The `SP1Host` is responsible for program execution and proving
#[derive(Clone)]
pub struct SP1Host {
    prover_options: ProverOptions,
    proving_key: SP1ProvingKey,
    prover_client: Arc<ProverClient>,
    vkey: SP1VerifyingKey,
    elf: Vec<u8>,
}

impl ZKVMHost for SP1Host {
    type Input<'a> = SP1ProofInputBuilder;
    fn init(guest_code: Vec<u8>, prover_options: ProverOptions) -> Self {
        let prover_client = ProverClient::new();
        let (proving_key, vkey) =
            get_proving_keys(&prover_client, &guest_code, prover_options.use_cached_keys);

        SP1Host {
            prover_options,
            prover_client: Arc::new(prover_client),
            proving_key,
            vkey,
            elf: guest_code,
        }
    }

    fn prove<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
    ) -> anyhow::Result<(ProofWithMetadata, VerificationKey)> {
        // Init the prover
        if self.prover_options.use_mock_prover {
            std::env::set_var("SP1_PROVER", "mock");
            let mock_proof = Proof::new(vec![]);
            let mock_proof_with_metadata =
                ProofWithMetadata::new("mock_proof".to_owned(), mock_proof, None);
            let mock_vk = VerificationKey::new(vec![]);
            return Ok((mock_proof_with_metadata, mock_vk));
        }

        let client = self.prover_client.clone();

        // Generate unique ID for the proof
        let mut input = bincode::serialize(&prover_input)?;
        input.extend_from_slice(&self.vkey.hash_bytes());
        let proof_id = format!("{}", strata_primitives::hash::raw(&input));

        // Start proving
        let mut prover = client.prove(&self.proving_key, prover_input.clone());
        if self.prover_options.enable_compression {
            prover = prover.compressed();
        }
        if self.prover_options.stark_to_snark_conversion {
            prover = prover.groth16();
        }

        let (remote_id, proof_data) = if client.prover.id() == ProverType::Network {
            // SAFETY: We use `unsafe` to downcast the trait object to `NetworkProver` because
            // `Prover` doesn't implement `Any`, so we can't use safe downcasting.
            // Since `client.prover` is initialized as `NetworkProver` when `SP1_PROVER ==
            // "network"`, this cast is valid in this context. The cast bypasses Rust's
            // type checks, so we must ensure the environment variable is set correctly
            // to avoid undefined behavior.
            let network_prover =
                unsafe { &*(client.prover.as_ref() as *const _ as *const NetworkProver) };

            let mode = match (
                self.prover_options.enable_compression,
                self.prover_options.stark_to_snark_conversion,
            ) {
                (_, true) => ProofMode::Groth16,
                (true, _) => ProofMode::Compressed,
                (_, _) => ProofMode::default(),
            };

            let remote_id =
                block_on(network_prover.request_proof(&self.elf, prover_input.clone(), mode))?;

            let proof_data: SP1ProofWithPublicValues =
                block_on(network_prover.wait_proof(&remote_id, None))?;

            // TODO: move saving proof into prover_options
            // let filename: String = format!("{}.{}proof", remote_id, self.prover_options);
            // let mut file = File::create(filename).unwrap();
            // file.write_all(&bincode::serialize(&proof_data).unwrap())
            //     .unwrap();

            (Some(remote_id), proof_data)
        } else {
            let proof_data = prover.run()?;
            (None, proof_data)
        };

        // Proof serialization
        let verification_key = bincode::serialize(&self.vkey)?;
        let proof = Proof::new(bincode::serialize(&proof_data)?);

        Ok((
            ProofWithMetadata::new(proof_id, proof, remote_id),
            VerificationKey(verification_key),
        ))
    }

    fn simulate_and_extract_output<'a, T: DeserializeOwned + serde::Serialize>(
        &self,
        prover_input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
        filename: &str,
    ) -> anyhow::Result<(u64, T)> {
        // Init the prover
        if self.prover_options.use_mock_prover {
            std::env::set_var("TRACE_FILE", filename);
        }

        let executor = self.prover_client.execute(&self.elf, prover_input.clone());
        let (mut ser_output, report) = executor.run()?;
        let output: T = ser_output.read();

        Ok((report.total_instruction_count(), output))
    }

    fn simulate_and_extract_output_borsh<'a, T: BorshSerialize + BorshDeserialize>(
        &self,
        prover_input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
        filename: &str,
    ) -> anyhow::Result<(u64, T)> {
        // Init the prover
        if self.prover_options.use_mock_prover {
            std::env::set_var("TRACE_FILE", filename);
        }

        let executor = self.prover_client.execute(&self.elf, prover_input.clone());
        let (ser_output, report) = executor.run()?;
        let output: T = borsh::from_slice(ser_output.as_slice())?;

        Ok((report.total_instruction_count(), output))
    }

    fn get_verification_key(&self) -> VerificationKey {
        let verification_key = bincode::serialize(&self.vkey).unwrap();
        VerificationKey::new(verification_key)
    }
}

// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(test)]
#[cfg(not(debug_assertions))]
mod tests {

    use std::{fs::File, io::Write};

    use sp1_sdk::{HashableKey, SP1Stdin, SP1VerifyingKey};
    use strata_zkvm::ZKVMVerifier;

    use super::*;
    use crate::SP1Verifier;

    // Adding compiled guest code `TEST_ELF` to save the build time
    // #![no_main]
    // sp1_zkvm::entrypoint!(main);
    // fn main() {
    //     let n = sp1_zkvm::io::read::<u32>();
    //     sp1_zkvm::io::commit(&n);
    // }
    const TEST_ELF: &[u8] = include_bytes!("../tests/elf/riscv32im-succinct-zkvm-elf");

    fn get_zkvm_input(input: u32) -> SP1Stdin {
        SP1ProofInputBuilder::new()
            .write(&input)
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn test_simulation() {
        let input = 1;
        let prover_input = get_zkvm_input(input);

        let zkvm = SP1Host::init(TEST_ELF.to_vec(), ProverOptions::default());
        let trace_file = "test_trace_file.log";
        let (cycles, output): (u64, u32) = zkvm
            .simulate_and_extract_output(prover_input, trace_file)
            .expect("Simulation failed");

        // assert simulation works
        assert_eq!(input, output);
        assert_eq!(cycles, 4791);
    }

    #[test]
    fn test_mock_prover() {
        let input = 1;
        let prover_input = get_zkvm_input(input);

        // assert proof generation works
        let zkvm = SP1Host::init(TEST_ELF.to_vec(), ProverOptions::default());
        let (proof, vk) = zkvm.prove(prover_input).expect("Failed to generate proof");

        // assert proof verification works
        SP1Verifier::verify(&vk, proof.proof()).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out: u32 = SP1Verifier::extract_public_output(proof.proof()).expect(
            "Failed to extract public
    outputs",
        );
        assert_eq!(input, out)
    }

    #[test]
    fn test_mock_prover_with_public_param() {
        let input = 1;
        let prover_input = get_zkvm_input(input);

        // assert proof generation works
        let zkvm = SP1Host::init(TEST_ELF.to_vec(), ProverOptions::default());
        let (proof, vk) = zkvm.prove(prover_input).expect("Failed to generate proof");

        // assert proof verification works
        SP1Verifier::verify_with_public_params(&vk, input, proof.proof())
            .expect("Proof verification failed");
    }

    #[test]
    fn test_groth16_proof_generation() {
        let input = 1;
        sp1_sdk::utils::setup_logger();

        let prover_input = get_zkvm_input(input);

        // Prover Options to generate Groth16 proof
        let prover_options = ProverOptions {
            enable_compression: false,
            use_mock_prover: false,
            stark_to_snark_conversion: true,
            use_cached_keys: true,
        };
        let zkvm = SP1Host::init(TEST_ELF.to_vec(), prover_options);

        // assert proof generation works
        let (proof, vk) = zkvm.prove(prover_input).expect("Failed to generate proof");

        let vk: SP1VerifyingKey = bincode::deserialize(vk.as_bytes()).unwrap();

        // Note: For the fixed ELF and fixed SP1 version, the vk is fixed
        assert_eq!(
            vk.bytes32(),
            "0x00b01ae596b4e51843484ff71ccbd0dd1a030af70b255e6b9aad50b81d81266f"
        );

        let filename = "./tests/proofs/proof-groth16.bin";
        let mut file = File::create(filename).unwrap();
        file.write_all(&bincode::serialize(&proof).unwrap())
            .unwrap();
    }
}
