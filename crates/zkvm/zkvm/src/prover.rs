use crate::{host::ZkVmHost, input::ZkVmInputBuilder, proof::Proof, ProofType};

pub trait ZkVmProver {
    type Input;
    type Output;

    fn proof_type() -> ProofType;

    /// Prepares the input for the zkVM.
    fn prepare_input<'a, B>(input: &'a Self::Input) -> anyhow::Result<B::Input>
    where
        B: ZkVmInputBuilder<'a>;

    /// Processes the proof to produce the final output.
    fn process_output<H>(proof: &Proof, host: &H) -> anyhow::Result<Self::Output>
    where
        H: ZkVmHost;

    /// Proves the computation using any zkVM host.
    fn prove<'a, H>(input: &'a Self::Input, host: &H) -> anyhow::Result<(Proof, Self::Output)>
    where
        H: ZkVmHost,
        H::Input<'a>: ZkVmInputBuilder<'a>,
    {
        // Prepare the input using the host's input builder.
        let zkvm_input = Self::prepare_input::<H::Input<'a>>(input)?;

        // Use the host to prove.
        let (proof, _) = host.prove(zkvm_input, Self::proof_type())?;

        // Process and return the output using the verifier.
        let output = Self::process_output(&proof, host)?;

        Ok((proof, output))
    }
}
