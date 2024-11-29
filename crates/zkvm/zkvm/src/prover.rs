use crate::{
    host::ZkVmHost, input::ZkVmInputBuilder, ProofReceipt, ProofType, PublicValues, ZkVmResult,
};

pub trait ZkVmProver {
    type Input;
    type Output;

    fn proof_type() -> ProofType;

    /// Prepares the input for the zkVM.
    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmResult<B::Input>
    where
        B: ZkVmInputBuilder<'a>;

    /// Processes the [`PublicValues`] to produce the final output.
    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: ZkVmHost;

    /// Proves the computation using any zkVM host.
    fn prove<'a, H>(input: &'a Self::Input, host: &H) -> ZkVmResult<ProofReceipt>
    where
        H: ZkVmHost,
        H::Input<'a>: ZkVmInputBuilder<'a>,
    {
        // Prepare the input using the host's input builder.
        let zkvm_input = Self::prepare_input::<H::Input<'a>>(input)?;

        // Use the host to prove.
        let receipt = host.prove(zkvm_input, Self::proof_type())?;

        // Process output to see if we are getting the expected output.
        let _ = Self::process_output::<H>(&receipt.public_values)?;

        Ok(receipt)
    }
}
