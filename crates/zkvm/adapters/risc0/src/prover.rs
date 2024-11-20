use risc0_zkvm::{compute_image_id, default_prover, ProverOpts};
use strata_zkvm::{Proof, ProverOptions, VerificationKey, ZkVmHost, ZkVmInputBuilder};

use crate::input::Risc0ProofInputBuilder;

/// A host for the `Risc0` zkVM that stores the guest program in ELF format
/// The `Risc0Host` is responsible for program execution and proving

#[derive(Clone)]
pub struct Risc0Host {
    elf: Vec<u8>,
    prover_options: ProverOptions,
}

impl Risc0Host {
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

impl ZkVmHost for Risc0Host {
    type Input<'a> = Risc0ProofInputBuilder<'a>;

    fn init(guest_code: Vec<u8>, prover_options: ProverOptions) -> Self {
        Risc0Host {
            elf: guest_code,
            prover_options,
        }
    }

    fn prove<'a>(
        &self,
        prover_input: <Self::Input<'a> as ZkVmInputBuilder<'a>>::Input,
    ) -> anyhow::Result<(Proof, VerificationKey)> {
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

        // Proof serialization
        let serialized_proof = bincode::serialize(&proof_info.receipt)?;
        Ok((
            Proof::new(serialized_proof),
            VerificationKey(verification_key),
        ))
    }

    fn get_verification_key(&self) -> VerificationKey {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use strata_zkvm::ZkVmVerifier;

    use super::*;
    use crate::{Risc0ProofInputBuilder, Risc0Verifier};

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
        let zkvm = Risc0Host::init(TEST_ELF.to_vec(), ProverOptions::default());

        // prepare input
        let mut zkvm_input_builder = Risc0ProofInputBuilder::new();
        zkvm_input_builder.write_serde(&input).unwrap();
        let zkvm_input = zkvm_input_builder.build().unwrap();

        // assert proof generation works
        let (proof, vk) = zkvm.prove(zkvm_input).expect("Failed to generate proof");

        // assert proof verification works
        Risc0Verifier::verify(&vk, &proof).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out: u32 = Risc0Verifier::extract_serde_public_output(&proof)
            .expect("Failed to extract public outputs");
        assert_eq!(input, out)
    }

    #[test]
    fn test_mock_prover_with_public_param() {
        let input: u32 = 1;
        let zkvm = Risc0Host::init(TEST_ELF.to_vec(), ProverOptions::default());

        // prepare input
        let mut zkvm_input_builder = Risc0ProofInputBuilder::new();
        zkvm_input_builder.write_serde(&input).unwrap();
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

        let zkvm = Risc0Host::init(TEST_ELF.to_vec(), prover_options);

        // prepare input
        let zkvm_input = Risc0ProofInputBuilder::new()
            .write_serde(&input)
            .unwrap()
            .build()
            .unwrap();

        // assert proof generation works
        let (proof, vk) = zkvm.prove(zkvm_input).expect("Failed to generate proof");

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
