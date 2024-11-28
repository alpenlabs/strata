use std::cell::{Cell, RefCell};

use strata_zkvm::{Proof, ZkVmEnv};

/// A native implementation of the [`ZkVmEnv`]
///
/// This uses interior mutability patterns ([`Cell`] and [`RefCell`]) to conform
/// to the [`ZkVmEnv`] trait, which requires methods to take an immutable reference to `self`.
#[derive(Debug, Clone)]
pub struct NativeMachine {
    /// A vector containing chunks of serialized input data.
    ///
    /// Each element in the vector represents a separate input that can be deserialized and
    /// processed.
    pub inputs: Vec<Vec<u8>>,

    /// A pointer to the current position in the `input` vector.
    ///
    /// Uses `Cell` for interior mutability, allowing `input_ptr` to be incremented within methods
    /// that have an immutable reference to `self`. This keeps track of the next input to read.
    pub input_ptr: Cell<usize>,

    /// A vector for collecting serialized output data chunks.
    ///
    /// Wrapped in a `RefCell` to allow mutable access even when `self` is immutable. This stores
    /// the outputs produced.
    pub output: RefCell<Vec<u8>>,
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
        Self::new()
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
        self.output.borrow_mut().extend_from_slice(raw_output);
    }

    fn commit_serde<T: serde::Serialize>(&self, output: &T) {
        let bytes = bincode::serialize(output).expect("bincode serialization failed");
        self.commit_buf(&bytes);
    }

    fn verify_groth16_proof(
        &self,
        _proof: &Proof,
        _verification_key: &[u8],
        _public_params_raw: &[u8],
    ) {
    }

    fn verify_native_proof(&self, _vk_digest: &[u32; 8], _public_values: &[u8]) {}

    fn read_verified_serde<T: serde::de::DeserializeOwned>(&self, _vk_digest: &[u32; 8]) -> T {
        self.read_serde()
    }
}
