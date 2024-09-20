use risc0_zkvm::{
    compute_image_id, get_prover_server, sha::Digest, ExecutorEnv, ExecutorImpl, ProverOpts,
    Receipt, VerifierContext,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::to_vec;
use strata_zkvm::{Proof, ProverInput, ProverOptions, VerificationKey, ZKVMHost, ZKVMVerifier};

/// A host for the `RiscZero` zkVM that stores the guest program in ELF format
/// The `RiscZeroHost` is responsible for program execution and proving

#[derive(Clone)]
pub struct RiscZeroHost {
    elf: Vec<u8>,
    prover_options: ProverOptions,
}

impl RiscZeroHost {
    fn determine_prover_options(&self) -> ProverOpts {
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
    fn init(guest_code: Vec<u8>, prover_options: ProverOptions) -> Self {
        RiscZeroHost {
            elf: guest_code,
            prover_options,
        }
    }

    fn prove<T: serde::Serialize>(
        &self,
        prover_input: &ProverInput<T>,
    ) -> anyhow::Result<(Proof, VerificationKey)> {
        if self.prover_options.use_mock_prover {
            std::env::set_var("RISC0_DEV_MODE", "true");
        }

        let mut env_builder = ExecutorEnv::builder();
        for input in &prover_input.inputs {
            env_builder.write(input)?;
        }
        for serialized_item in &prover_input.serialized_inputs {
            env_builder.write(&(serialized_item.len() as u32))?;
            env_builder.write_slice(serialized_item);
        }

        // Learn more about assumption and proof compositions at https://dev.risczero.com/api/zkvm/composition
        for agg_input in &prover_input.agg_inputs {
            let receipt: Receipt = bincode::deserialize(agg_input.proof().as_bytes())?;
            let vk: Digest = bincode::deserialize(agg_input.vk().as_bytes())?;

            // `add_assumption` makes the receipt to be verified available to the prover.
            env_builder.add_assumption(receipt);

            // Write the verification key of the program that'll be proven in the guest.
            // Note: The vkey is written here so we don't have to hardcode it in guest code.
            // TODO: This should be fixed once the guest code is finalized
            env_builder.write(&vk)?;
        }
        let env = env_builder.build()?;

        // Setup the prover
        let opts = self.determine_prover_options();
        let prover = get_prover_server(&opts)?;

        // Setup verification key
        let program_id = compute_image_id(&self.elf)?;
        let verification_key = bincode::serialize(&program_id)?;

        // Generate the session
        let mut exec = ExecutorImpl::from_elf(env, &self.elf)?;
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

/// A verifier for the `RiscZero` zkVM, responsible for verifying proofs generated by the host
pub struct Risc0Verifier;

impl ZKVMVerifier for Risc0Verifier {
    fn verify(verification_key: &VerificationKey, proof: &Proof) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        let vk: Digest = bincode::deserialize(&verification_key.0)?;
        receipt.verify(vk)?;
        Ok(())
    }

    fn verify_with_public_params<T: serde::Serialize + serde::de::DeserializeOwned>(
        verification_key: &VerificationKey,
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        let vk: Digest = bincode::deserialize(&verification_key.0)?;
        receipt.verify(vk)?;

        let actual_public_parameter: T = receipt.journal.decode()?;

        // TODO: use custom ZKVM error
        anyhow::ensure!(
            to_vec(&actual_public_parameter)? == to_vec(&public_params)?,
            "Failed to verify proof given the public param"
        );

        Ok(())
    }

    fn extract_public_output<T: Serialize + DeserializeOwned>(proof: &Proof) -> anyhow::Result<T> {
        let receipt: Receipt = bincode::deserialize(proof.as_bytes())?;
        Ok(receipt.journal.decode()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // assert proof generation works
        let mut prover_input = ProverInput::new();
        prover_input.write(input);
        let (proof, vk) = zkvm.prove(&prover_input).expect("Failed to generate proof");

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

        // assert proof generation works
        let mut prover_input = ProverInput::new();
        prover_input.write(input);
        let (proof, vk) = zkvm.prove(&prover_input).expect("Failed to generate proof");

        // assert proof verification works
        Risc0Verifier::verify_with_public_params(&vk, input, &proof)
            .expect("Proof verification failed");
    }
}
