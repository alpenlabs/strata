use risc0_zkvm::{compute_image_id, default_prover, ProverOpts};
use strata_zkvm::{
    Proof, ProofWithMetadata, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
};

use crate::input::RiscZeroProofInputBuilder;

/// A host for the `RiscZero` zkVM that stores the guest program in ELF format
/// The `RiscZeroHost` is responsible for program execution and proving

#[derive(Clone)]
pub struct RiscZeroHost {
    elf: Vec<u8>,
    prover_options: ProverOptions,
}

impl RiscZeroHost {
    pub(crate) fn determine_prover_options(&self) -> ProverOpts {
        if self.prover_options.stark_to_snark_conversion {
            ProverOpts::groth16()
        } else if self.prover_options.enable_compression {
            ProverOpts::succinct()
        } else {
            ProverOpts::default()
        }
    }
}

impl ZKVMHost for RiscZeroHost {
    type Input<'a> = RiscZeroProofInputBuilder<'a>;

    fn init(guest_code: Vec<u8>, prover_options: ProverOptions) -> Self {
        RiscZeroHost {
            elf: guest_code,
            prover_options,
        }
    }

    fn prove<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
    ) -> anyhow::Result<(ProofWithMetadata, VerificationKey)> {
        if self.prover_options.use_mock_prover {
            std::env::set_var("RISC0_DEV_MODE", "true");
        }

        // Setup the prover
        let opts = self.determine_prover_options();
        // let prover = get_prover_server(&opts)?;
        let prover = default_prover();

        // Setup verification key
        let program_id = compute_image_id(&self.elf)?;
        let verification_key = bincode::serialize(&program_id)?;

        // Generate the session
        // let mut exec = ExecutorImpl::from_elf(prover_input, &self.elf)?;
        // let session = exec.run()?;

        // Generate the proof
        // let ctx = VerifierContext::default();
        // let proof_info = prover.prove_session(&ctx, &session)?;
        let proof_info = prover.prove_with_opts(prover_input, &self.elf, &opts)?;

        // Generate unique ID for the proof
        let mut input = program_id.as_bytes().to_vec();
        input.extend_from_slice(&proof_info.receipt.journal.bytes);
        let proof_id = format!("{}", strata_primitives::hash::raw(&input));

        // Proof serialization
        let proof = Proof::new(bincode::serialize(&proof_info.receipt)?);
        Ok((
            ProofWithMetadata::new(proof_id, proof, None),
            VerificationKey(verification_key),
        ))
    }

    fn simulate_and_extract_output<'a, T: serde::Serialize + serde::de::DeserializeOwned>(
        &self,
        _prover_input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
        _filename: &str,
    ) -> anyhow::Result<(u64, T)> {
        todo!();
    }

    fn simulate_and_extract_output_borsh<'a, T: borsh::BorshSerialize + borsh::BorshDeserialize>(
        &self,
        _prover_input: <Self::Input<'a> as ZKVMInputBuilder<'a>>::Input,
        _filename: &str,
    ) -> anyhow::Result<(u64, T)> {
        todo!();
    }

    fn get_verification_key(&self) -> VerificationKey {
        todo!()
    }
}

#[cfg(test)]
#[cfg(all(feature = "prover", not(debug_assertions)))] // FIXME: This is working locally but tests failing in the CI.
mod tests {
    use std::{fs::File, io::Write};

    use strata_zkvm::ZKVMVerifier;

    use super::*;
    use crate::{Risc0Verifier, RiscZeroProofInputBuilder};

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
        let zkvm = RiscZeroHost::init(TEST_ELF.to_vec(), ProverOptions::default());

        // prepare input
        let mut zkvm_input_builder = RiscZeroProofInputBuilder::new();
        zkvm_input_builder.write(&input).unwrap();
        let zkvm_input = zkvm_input_builder.build().unwrap();

        // assert proof generation works
        let (proof, vk) = zkvm.prove(zkvm_input).expect("Failed to generate proof");

        // assert proof verification works
        Risc0Verifier::verify(&vk, &proof).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out: u32 = Risc0Verifier::extract_public_output(&proof).expect(
            "Failed to extract public
        outputs",
        );
        assert_eq!(input, out)
    }

    #[test]
    fn test_mock_prover_with_public_param() {
        let input: u32 = 1;
        let zkvm = RiscZeroHost::init(TEST_ELF.to_vec(), ProverOptions::default());

        // prepare input
        let mut zkvm_input_builder = RiscZeroProofInputBuilder::new();
        zkvm_input_builder.write(&input).unwrap();
        let zkvm_input = zkvm_input_builder.build().unwrap();

        // assert proof generation works
        let (proof, vk) = zkvm.prove(zkvm_input).expect("Failed to generate proof");

        // assert proof verification works
        Risc0Verifier::verify_with_public_params(&vk, input, &proof)
            .expect("Proof verification failed");
    }

    #[test]
    #[ignore]
    fn test_groth16_proof_gen() {
        // Initialize tracing. In order to view logs, run `RUST_LOG=info cargo run`
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
            .init();

        let input: u32 = 1;

        // Prover Options to generate Groth16 proof
        let prover_options = ProverOptions {
            enable_compression: false,
            use_mock_prover: false,
            stark_to_snark_conversion: true,
            use_cached_keys: true,
        };

        let zkvm = RiscZeroHost::init(TEST_ELF.to_vec(), prover_options);

        // prepare input
        let zkvm_input = RiscZeroProofInputBuilder::new()
            .write(&input)
            .unwrap()
            .build()
            .unwrap();

        // assert proof generation works
        let (proof, vk) = zkvm.prove(zkvm_input).expect("Failed to generate proof");

        let expected_vk = vec![
            10, 54, 13, 204, 148, 23, 239, 151, 171, 193, 81, 136, 44, 50, 212, 47, 131, 118, 33,
            162, 117, 207, 35, 7, 45, 14, 98, 169, 38, 223, 115, 214,
        ];

        let filename = "./tests/proofs/proof-groth16.bin";
        let mut file = File::create(filename).unwrap();
        file.write_all(proof.as_bytes()).unwrap();

        assert_eq!(vk.as_bytes(), expected_vk);
    }
}
