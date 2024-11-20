use std::cell::{Cell, RefCell};

use strata_zkvm::ZkVmEnv;

#[derive(Debug, Clone)]
pub struct NativeMachine {
    pub inputs: Vec<Vec<u8>>,
    pub input_ptr: Cell<usize>,
    pub output: RefCell<Vec<Vec<u8>>>,
}

impl NativeMachine {
    pub fn new() -> Self {
        Self {
            inputs: vec![],
            input_ptr: Cell::new(0),
            output: RefCell::new(vec![]),
        }
    }

    pub fn write_slice(&mut self, input: Vec<u8>) {
        self.inputs.push(input);
    }
}

impl Default for NativeMachine {
    fn default() -> Self {
        NativeMachine::new()
    }
}

impl ZkVmEnv for NativeMachine {
    fn read_buf(&self) -> Vec<u8> {
        // Get the current value of input_ptr
        let idx = self.input_ptr.get();

        let bytes = self.inputs[idx].clone();

        // Increment input_ptr
        self.input_ptr.set(idx + 1);

        bytes
    }

    fn read_serde<T: serde::de::DeserializeOwned>(&self) -> T {
        let bytes = self.read_buf();
        bincode::deserialize(&bytes).expect("bincode deserialization failed")
    }

    fn commit_buf(&self, raw_output: &[u8]) {
        self.output.borrow_mut().push(raw_output.to_vec());
    }

    fn commit_serde<T: serde::Serialize>(&self, output: &T) {
        let bytes = bincode::serialize(output).expect("bincode serialization failed");
        self.commit_buf(&bytes);
    }

    fn verify_groth16_proof(
        &self,
        _proof: &[u8],
        _verification_key: &[u8],
        _public_params_raw: &[u8],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn verify_native_proof(&self, _vk_digest: &[u32; 8], _public_values: &[u8]) {}

    fn read_verified_serde<T: serde::de::DeserializeOwned>(&self, _vk_digest: &[u32; 8]) -> T {
        self.read_serde()
    }
}
