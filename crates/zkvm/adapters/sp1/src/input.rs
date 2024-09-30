use anyhow::Ok;
use express_zkvm::{AggregationInput, Proof, ZKVMInputBuilder};
use sp1_sdk::{HashableKey, SP1Proof, SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey};

// A wrapper around SP1Stdin
pub struct SP1ProofInputBuilder(SP1Stdin);

impl<'a> ZKVMInputBuilder<'a> for SP1ProofInputBuilder {
    type Input = SP1Stdin;
    fn new() -> SP1ProofInputBuilder {
        SP1ProofInputBuilder(SP1Stdin::new())
    }

    fn write<T: serde::Serialize>(&mut self, item: &T) -> anyhow::Result<&mut Self> {
        self.0.write(item);
        Ok(self)
    }

    fn write_borsh<T: borsh::BorshSerialize>(&mut self, item: &T) -> anyhow::Result<&mut Self> {
        let slice = borsh::to_vec(item)?;
        self.write_serialized(&slice)
    }

    fn write_serialized(&mut self, item: &[u8]) -> anyhow::Result<&mut Self> {
        self.0.write_slice(item);
        Ok(self)
    }

    fn write_proof_with_vkey(&mut self, item: AggregationInput) -> anyhow::Result<&mut Self> {
        let proof: SP1ProofWithPublicValues = bincode::deserialize(item.proof().as_bytes())?;
        let vkey: SP1VerifyingKey = bincode::deserialize(item.vk().as_bytes())?;

        // Write the verification key and the public values of the program that'll be proven
        // inside zkVM.
        // Note: The vkey is written here so we don't have to hardcode it in guest code.
        // TODO: This should be fixed once the guest code is finalized
        self.0.write(&vkey.hash_u32());
        self.0.write(&proof.public_values);

        // Write the proofs.
        //
        // Note: this data will not actually be read by the aggregation program, instead it will
        // be witnessed by the prover during the recursive aggregation process
        // inside SP1 itself.
        match proof.proof {
            SP1Proof::Compressed(compressed_proof) => {
                self.0.write_proof(compressed_proof, vkey.vk);
            }
            _ => return Err(anyhow::anyhow!("can only handle compressed proofs")),
        }

        Ok(self)
    }

    fn write_proof(&mut self, _item: Proof) -> anyhow::Result<&mut Self> {
        todo!()
    }

    fn build(&mut self) -> anyhow::Result<Self::Input> {
        anyhow::Ok(self.0.clone())
    }
}
