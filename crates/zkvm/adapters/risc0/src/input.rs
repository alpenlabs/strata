use risc0_zkvm::{sha::Digest, ExecutorEnv, ExecutorEnvBuilder, Receipt};
use strata_zkvm::ZKVMInputBuilder;

pub struct RiscZeroProofInputBuilder<'a>(ExecutorEnvBuilder<'a>);

impl<'a> ZKVMInputBuilder<'a> for RiscZeroProofInputBuilder<'a> {
    type Input = ExecutorEnv<'a>;

    fn new() -> Self {
        let env_builder = ExecutorEnv::builder();
        Self(env_builder)
    }

    fn write<T: serde::Serialize>(&mut self, item: &T) -> anyhow::Result<&mut Self> {
        self.0.write(item)?;
        Ok(self)
    }

    fn write_borsh<T: borsh::BorshSerialize>(&mut self, item: &T) -> anyhow::Result<&mut Self> {
        let slice = borsh::to_vec(item)?;
        self.write_serialized(&slice)
    }

    fn write_serialized(&mut self, item: &[u8]) -> anyhow::Result<&mut Self> {
        let len = item.len() as u32;
        self.0.write(&len)?;
        self.0.write_slice(item);
        Ok(self)
    }

    fn write_proof(&mut self, item: strata_zkvm::AggregationInput) -> anyhow::Result<&mut Self> {
        // Learn more about assumption and proof compositions at https://dev.risczero.com/api/zkvm/composition
        let receipt: Receipt = bincode::deserialize(item.proof().as_bytes())?;
        let vk: Digest = bincode::deserialize(item.vk().as_bytes())?;

        // `add_assumption` makes the receipt to be verified available to the prover.
        self.0.add_assumption(receipt);

        // Write the verification key of the program that'll be proven in the guest.
        // Note: The vkey is written here so we don't have to hardcode it in guest code.
        // TODO: This should be fixed once the guest code is finalized
        self.0.write(&vk)?;
        Ok(self)
    }

    fn build(&mut self) -> anyhow::Result<Self::Input> {
        self.0.build()
    }
}
