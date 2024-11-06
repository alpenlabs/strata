use std::cell::{Cell, RefCell};

use strata_zkvm::ZkVm;

pub struct NativeMachine {
    pub input: Vec<Vec<u8>>,
    pub input_ptr: Cell<usize>,
    pub output: RefCell<Vec<Vec<u8>>>,
}

impl ZkVm for NativeMachine {
    fn read<T: serde::de::DeserializeOwned>(&self) -> T {
        let bytes = self.read_slice();
        bincode::deserialize(&bytes).expect("bincode deserialization failed")
    }

    fn read_slice(&self) -> Vec<u8> {
        // Get the current value of input_ptr
        let idx = self.input_ptr.get();

        let bytes = self.input[idx].clone();

        // Increment input_ptr
        self.input_ptr.set(idx + 1);

        bytes
    }

    fn commit<T: serde::Serialize>(&self, output: &T) {
        let bytes = bincode::serialize(output).expect("bincode serialization failed");
        self.commit_slice(&bytes);
    }

    fn commit_slice(&self, raw_output: &[u8]) {
        self.output.borrow_mut().push(raw_output.to_vec());
    }

    fn verify_groth16(
        &self,
        _proof: &[u8],
        _verification_key: &[u8],
        _public_params_raw: &[u8],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn verify_proof(&self, _vk_digest: &[u32; 8], _public_values: &[u8]) {}
}
