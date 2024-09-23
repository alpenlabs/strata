use express_zkvm::{Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};
use risc0_zkvm::{compute_image_id, get_prover_server, ExecutorImpl, ProverOpts, VerifierContext};

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
    ) -> anyhow::Result<(Proof, VerificationKey)> {
        if self.prover_options.use_mock_prover {
            std::env::set_var("RISC0_DEV_MODE", "true");
        }

        // Setup the prover
        let opts = self.determine_prover_options();
        let prover = get_prover_server(&opts)?;

        // Setup verification key
        let program_id = compute_image_id(&self.elf)?;
        let verification_key = bincode::serialize(&program_id)?;

        // Generate the session
        let mut exec = ExecutorImpl::from_elf(prover_input, &self.elf)?;
        let session = exec.run()?;

        // Generate the proof
        let ctx = VerifierContext::default();
        let proof_info = prover.prove_session(&ctx, &session)?;

        // Proof serialization
        let serialized_proof = bincode::serialize(&proof_info.receipt)?;
        Ok((
            Proof::new(serialized_proof),
            VerificationKey(verification_key),
        ))
    }
}

#[cfg(test)]
mod tests {
    use express_zkvm::ZKVMVerifier;

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
        let out: u32 =
            Risc0Verifier::extract_public_output(&proof).expect("Failed to extract public outputs");
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
}
