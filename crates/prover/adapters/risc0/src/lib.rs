use anyhow::Ok;
use risc0_zkvm::{get_prover_server, ExecutorEnv, ProverOpts, Receipt};
use serde_json::to_vec;
use zkvm::{Proof, ProverOptions, ZKVMHost, ZKVMVerifier};

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

    fn prove<T: serde::Serialize>(&self, input: T) -> anyhow::Result<Proof> {
        if self.prover_options.use_mock_prover {
            std::env::set_var("RISC0_DEV_MODE", "true");
        }

        let env = ExecutorEnv::builder().write(&input)?.build()?;
        let opts = self.determine_prover_options();

        let prover = get_prover_server(&opts)?;
        let proof = prover.prove(env, &self.elf)?.receipt;
        let serialized_proof = bincode::serialize(&proof)?;
        Ok(Proof(serialized_proof))
    }
}

pub struct Risc0Verifier;

impl ZKVMVerifier for Risc0Verifier {
    fn verify(program_id: [u32; 8], proof: &Proof) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(&proof.0)?;
        receipt.verify(program_id)?;
        Ok(())
    }

    fn extract_public_output<T: serde::de::DeserializeOwned>(proof: &Proof) -> anyhow::Result<T> {
        let receipt: Receipt = bincode::deserialize(&proof.0)?;
        Ok(receipt.journal.decode()?)
    }

    fn verify_with_public_params<T: serde::de::DeserializeOwned + serde::Serialize>(
        program_id: [u32; 8],
        public_params: T,
        proof: &Proof,
    ) -> anyhow::Result<()> {
        let receipt: Receipt = bincode::deserialize(&proof.0)?;
        receipt.verify(program_id)?;
        let actual_public_parameter: T = receipt.journal.decode()?;

        // TODO: Define custom ZKVM error message
        anyhow::ensure!(
            to_vec(&actual_public_parameter)? == to_vec(&public_params)?,
            "Failed to verify proof given the public param"
        );

        Ok(())
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
        let zkvm = RiscZeroHost::init(TEST_ELF.to_vec(), ProverOptions::default());

        // assert proof generation works
        let proof = zkvm.prove(input).expect("Failed to generate proof");

        // assert proof verification works
        Risc0Verifier::verify(TEST_ELF_PROGRAM_ID, &proof).expect("Proof verification failed");

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
        let proof = zkvm.prove(input).expect("Failed to generate proof");

        // assert proof verification works
        Risc0Verifier::verify_with_public_params(TEST_ELF_PROGRAM_ID, input, &proof)
            .expect("Proof verification failed");
    }
}
