use strata_zkvm::{AggregationInput, ProofReceipt, ZkVmInputBuilder, ZkVmResult};

use crate::env::NativeMachine;

pub struct NativeMachineInputBuilder(pub NativeMachine);

impl<'a> ZkVmInputBuilder<'a> for NativeMachineInputBuilder {
    type Input = NativeMachine;
    type ProofImpl = ProofReceipt;

    fn new() -> NativeMachineInputBuilder {
        Self(NativeMachine::new())
    }

    fn write_buf(&mut self, item: &[u8]) -> ZkVmResult<&mut Self> {
        self.0.write_slice(item.to_vec());
        Ok(self)
    }

    fn write_serde<T: serde::Serialize>(&mut self, item: &T) -> ZkVmResult<&mut Self> {
        let slice = bincode::serialize(&item)?;
        self.write_buf(&slice)
    }

    fn write_borsh<T: borsh::BorshSerialize>(&mut self, item: &T) -> ZkVmResult<&mut Self> {
        let slice = borsh::to_vec(item)?;
        self.write_buf(&slice)
    }

    fn write_proof(&mut self, item: &AggregationInput) -> ZkVmResult<&mut Self> {
        // For the native mode we only write the public values since the proof is expected to be
        // empty
        self.write_buf(item.receipt().public_values().as_bytes())
    }

    fn build(&mut self) -> ZkVmResult<Self::Input> {
        Ok(self.0.clone())
    }
}
