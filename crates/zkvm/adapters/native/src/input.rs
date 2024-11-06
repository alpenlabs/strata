use strata_zkvm::ZkVmInputBuilder;

use crate::zkvm::NativeMachine;

pub struct NativeMachineInputBuilder(pub NativeMachine);

impl<'a> ZkVmInputBuilder<'a> for NativeMachineInputBuilder {
    type Input = NativeMachine;

    fn new() -> NativeMachineInputBuilder {
        Self(NativeMachine::new())
    }

    fn write_buf(&mut self, item: &[u8]) -> anyhow::Result<&mut Self> {
        self.0.write_slice(item.to_vec());
        Ok(self)
    }

    fn write_serde<T: serde::Serialize>(&mut self, item: &T) -> anyhow::Result<&mut Self> {
        let slice = bincode::serialize(&item)?;
        self.write_buf(&slice)
    }

    fn write_borsh<T: borsh::BorshSerialize>(&mut self, item: &T) -> anyhow::Result<&mut Self> {
        let slice = borsh::to_vec(item)?;
        self.write_buf(&slice)
    }

    fn write_proof(&mut self, item: strata_zkvm::AggregationInput) -> anyhow::Result<&mut Self> {
        // TODO: figure this out
        self.write_buf(&[1])
    }

    fn build(&mut self) -> anyhow::Result<Self::Input> {
        anyhow::Ok(self.0.clone())
    }
}
