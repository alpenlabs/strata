use anyhow::Ok;
use risc0_zkvm::{get_prover_server, ExecutorEnv, ProverOpts, Receipt};
use zkvm::{Proof, ProverOptions, ZKVMHost, ZKVMVerifier};

pub struct RiscZeroHost {
    elf: Vec<u8>,
    inputs: Vec<u32>,
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
            inputs: Vec::new(),
            prover_options,
        }
    }

    fn prove(&self) -> anyhow::Result<Proof> {
        if self.prover_options.use_mock_prover {
            std::env::set_var("RISC0_DEV_MODE", "true");
        }

        let env = ExecutorEnv::builder().write(&self.inputs)?.build()?;
        let opts = self.determine_prover_options();

        let prover = get_prover_server(&opts)?;
        let proof = prover.prove(env, &self.elf)?.receipt;
        let serialized_proof = bincode::serialize(&proof)?;
        Ok(Proof(serialized_proof))
    }

    fn add_input<T: serde::Serialize>(&mut self, item: T) {
        let mut serializer = risc0_zkvm::serde::Serializer::new(&mut self.inputs);
        item.serialize(&mut serializer)
            .expect("Risc0 hint serialization is infallible");
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
}

#[cfg(test)]
mod tests {
    use zkvm::ProverOptions;

    use super::*;

    // Addding compiled guest code `TEST_ELF` to save the build time
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
    fn risc0_test_mock_prover() {
        let input: u32 = 1;
        let mut zkvm = RiscZeroHost::init(TEST_ELF.to_vec(), ProverOptions::default());
        zkvm.add_input(input);

        // assert proof generation works
        let proof = zkvm.prove().expect("Failed to generate proof");
        
        // assert proof verification works
        Risc0Verifier::verify(TEST_ELF_PROGRAM_ID, &proof).expect("Proof verification failed");

        // assert public outputs extraction from proof  works
        let out:u32 = Risc0Verifier::extract_public_output(&proof).expect("Failed to extract public outputs");
        assert_eq!(input, out)
    }
}
