use sp1_sdk::{SP1Proof, SP1Stdin, SP1VerifyingKey};
use strata_zkvm::{AggregationInput, ZkVmError, ZkVmInputBuilder, ZkVmResult};

use crate::proof::SP1ProofReceipt;

// A wrapper around SP1Stdin
pub struct SP1ProofInputBuilder(SP1Stdin);

impl<'a> ZkVmInputBuilder<'a> for SP1ProofInputBuilder {
    type Input = SP1Stdin;
    type ZkVmProofReceipt = SP1ProofReceipt;

    fn new() -> SP1ProofInputBuilder {
        SP1ProofInputBuilder(SP1Stdin::new())
    }

    fn write_serde<T: serde::Serialize>(&mut self, item: &T) -> ZkVmResult<&mut Self> {
        self.0.write(item);
        Ok(self)
    }

    fn write_borsh<T: borsh::BorshSerialize>(&mut self, item: &T) -> ZkVmResult<&mut Self> {
        let slice = borsh::to_vec(item)?;
        self.write_buf(&slice)
    }

    fn write_buf(&mut self, item: &[u8]) -> ZkVmResult<&mut Self> {
        self.0.write_slice(item);
        Ok(self)
    }

    fn write_proof(&mut self, item: &AggregationInput) -> ZkVmResult<&mut Self> {
        let receipt: SP1ProofReceipt = item.receipt().into();
        let vkey: SP1VerifyingKey = bincode::deserialize(item.vk().as_bytes())?;

        // Write the public values of the program that'll be proven inside zkVM.
        self.0
            .write_slice(receipt.as_ref().public_values.as_slice());

        // Write the proofs.
        //
        // Note: this data will not actually be read by the aggregation program, instead it will
        // be witnessed by the prover during the recursive aggregation process
        // inside SP1 itself.
        match receipt.inner().proof {
            SP1Proof::Compressed(compressed_proof) => {
                self.0.write_proof(*compressed_proof, vkey.vk);
            }
            _ => {
                return Err(ZkVmError::InputError(
                    "SP1 can only handle compressed proof".to_string(),
                ))
            }
        }

        Ok(self)
    }

    fn build(&mut self) -> ZkVmResult<Self::Input> {
        Ok(self.0.clone())
    }
}
