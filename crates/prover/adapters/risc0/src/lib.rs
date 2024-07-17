use bincode;
use risc0_zkvm::{default_prover, ExecutorEnv};
use zkvm::{Proof, ProverOptions, ZKVMHost};

pub struct RiscZeroZKVM {
    elf: Vec<u8>,
    inputs: Vec<u32>,
    prover_options: ProverOptions,
}

impl ZKVMHost for RiscZeroZKVM {
    fn init(guest_code: Vec<u8>, prover_options: zkvm::ProverOptions) -> Self {
        RiscZeroZKVM {
            elf: guest_code,
            inputs: Vec::new(),
            prover_options,
        }
    }

    fn prove(&self) -> anyhow::Result<Proof> {
        // Set DEV mode flag for the internal prover server
        if self.prover_options.use_mock_prover {
            std::env::set_var("RISC0_DEV_MODE", "true");
        }

        // Initialize the prover
        let env = ExecutorEnv::builder().write(&self.inputs)?.build()?;
        let prover = default_prover();
        let proof = prover.prove(env, &self.elf)?.receipt;

        let seralized_proof = bincode::serialize(&proof)?;
        Ok(Proof(seralized_proof))
    }

    fn add_input<T: serde::Serialize>(&mut self, item: T) {
        let mut serializer = risc0_zkvm::serde::Serializer::new(&mut self.inputs);
        item.serialize(&mut serializer)
            .expect("Risc0 hint serialization is infallible");
    }
}

#[cfg(test)]
mod tests {
    use zkvm::ProverOptions;

    use super::*;

    const TEST_ELF: &[u8] = include_bytes!("../elf/risc0-zkvm-elf");

    #[test]
    fn risc0_test_mock_prover() {
        let input: u32 = 10;
        
        let mut zkvm = RiscZeroZKVM::init(TEST_ELF.to_vec(), ProverOptions::default());
        zkvm.add_input(input);

        let proof = zkvm.prove();
        assert!(proof.is_ok())
    }
}
