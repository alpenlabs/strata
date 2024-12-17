use risc0_zkvm::{sha::Digest, ExecutorEnv, ExecutorEnvBuilder};
use strata_zkvm::{
    AggregationInput, DataFormatError, ZkVmInputBuilder, ZkVmInputError, ZkVmInputResult,
    ZkVmVerificationKeyError,
};

use crate::proof::Risc0ProofReceipt;

pub struct Risc0ProofInputBuilder<'a>(ExecutorEnvBuilder<'a>);

impl<'a> ZkVmInputBuilder<'a> for Risc0ProofInputBuilder<'a> {
    type Input = ExecutorEnv<'a>;
    type ZkVmProofReceipt = Risc0ProofReceipt;

    fn new() -> Self {
        let env_builder = ExecutorEnv::builder();
        Self(env_builder)
    }

    fn write_serde<T: serde::Serialize>(&mut self, item: &T) -> ZkVmInputResult<&mut Self> {
        self.0
            .write(item)
            .map_err(|e| ZkVmInputError::DataFormat(DataFormatError::Serde(e.to_string())))?;
        Ok(self)
    }

    fn write_borsh<T: borsh::BorshSerialize>(&mut self, item: &T) -> ZkVmInputResult<&mut Self> {
        let slice = borsh::to_vec(item)?;
        self.write_buf(&slice)
    }

    // TODO: replace this with `write_frame` once the API stabilizies
    fn write_buf(&mut self, item: &[u8]) -> ZkVmInputResult<&mut Self> {
        let len = item.len() as u32;
        self.0
            .write(&len)
            .map_err(|e| ZkVmInputError::DataFormat(DataFormatError::Serde(e.to_string())))?;
        self.0.write_slice(item);
        Ok(self)
    }

    fn write_proof(&mut self, item: &AggregationInput) -> ZkVmInputResult<&mut Self> {
        // Learn more about assumption and proof compositions at https://dev.risczero.com/api/zkvm/composition
        let receipt: Risc0ProofReceipt = item
            .receipt()
            .try_into()
            .map_err(ZkVmInputError::ProofReceipt)?;
        let vk: Digest = item
            .vk()
            .as_bytes()
            .try_into()
            .map_err(|_| ZkVmVerificationKeyError::InvalidVerificationKeySize)
            .map_err(ZkVmInputError::VerificationKey)?;

        // Write the verification key of the program that'll be proven in the guest.
        // Note: The vkey is written here so we don't have to hardcode it in guest code.
        // TODO: This should be fixed once the guest code is finalized
        self.write_buf(&receipt.as_ref().journal.bytes)?;
        self.0
            .write(&vk)
            .map_err(|e| ZkVmInputError::DataFormat(DataFormatError::Serde(e.to_string())))?;

        self.0.add_assumption(receipt.inner());

        Ok(self)
    }

    fn build(&mut self) -> ZkVmInputResult<Self::Input> {
        self.0
            .build()
            .map_err(|e| ZkVmInputError::InputBuild(e.to_string()))
    }
}
