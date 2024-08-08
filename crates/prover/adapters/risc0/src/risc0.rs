use risc0_zkvm::{
    compute_image_id, get_prover_server, sha::Digest, ExecutorEnv, ExecutorImpl, ProverOpts,
    Receipt, VerifierContext,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::to_vec;
use zkvm::{Proof, ProverOptions, VerifcationKey, ZKVMHost, ZKVMVerifier};

pub struct RiscZeroHost {
    elf: Vec<u8>,
    _inputs: Vec<u8>,
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
    fn init(guest_code: Vec<u8>, prover_options: zkvm::ProverOptions) -> Self {
        RiscZeroHost {
            elf: guest_code,
            _inputs: Vec::new(),
            prover_options,
        }
    }

    fn prove<T: serde::Serialize>(&self, item: T) -> anyhow::Result<(Proof, VerifcationKey)> {
        if self.prover_options.use_mock_prover {
            std::env::set_var("RISC0_DEV_MODE", "true");
        }

        // Setup the prover
        let env = ExecutorEnv::builder().write(&item)?.build()?;
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

        // Proof seralization
        let serialized_proof = bincode::serialize(&proof_info.receipt)?;
        Ok((Proof(serialized_proof), VerifcationKey(verification_key)))
    }
}

pub struct Risc0Verifier;

impl ZKVMVerifier for Risc0Verifier {
    fn verify(verification_key: &VerifcationKey, proof: &Proof) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(&proof.0)?;
        let vk: Digest = bincode::deserialize(&verification_key.0)?;
        receipt.verify(vk)?;
        Ok(())
    }

    fn verify_with_public_params<T: serde::Serialize + serde::de::DeserializeOwned>(
        verification_key: &VerifcationKey,
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(&proof.0)?;
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
        let receipt: Receipt = bincode::deserialize(&proof.0)?;
        Ok(receipt.journal.decode()?)
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

    #[test]
    fn test_mock_prover() {
        let input: u32 = 1;
        let zkvm = RiscZeroHost::init(TEST_ELF.to_vec(), ProverOptions::default());

        // assert proof generation works
        let (proof, vk) = zkvm.prove(input).expect("Failed to generate proof");

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
        let (proof, vk) = zkvm.prove(input).expect("Failed to generate proof");

        // assert proof verification works
        Risc0Verifier::verify_with_public_params(&vk, input, &proof)
            .expect("Proof verification failed");
    }
}
