use core::fmt;

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
    fn process_output<H>(proof: &Proof) -> anyhow::Result<Self::Output>
    where
        H: ZkVmHost;

    /// Proves the computation using any zkVM host.
    fn prove<'a, H, V>(input: &'a Self::Input, host: &H) -> anyhow::Result<(Proof, Self::Output)>
    where
        H: ZkVmHost,
        H::Input<'a>: ZkVmInputBuilder<'a>,
    {
        // Prepare the input using the host's input builder.
        let zkvm_input = Self::prepare_input::<H::Input<'a>>(input)?;

        // Use the host to prove.
        let (proof, _) = host.prove(zkvm_input, Self::proof_type())?;

        // Process and return the output using the verifier.
        let output = Self::process_output::<H>(&proof)?;

        Ok((proof, output))
    }
}

/// Prover config of the ZKVM Host
#[derive(Debug, Clone, Copy)]
pub struct ProverOptions {
    pub enable_compression: bool,
    pub use_mock_prover: bool,
    pub stark_to_snark_conversion: bool,
    pub use_cached_keys: bool,
    pub proof_type: ProofType,
}

// Compact representation of the prover options
// Can be used to identify the saved proofs
impl fmt::Display for ProverOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut has_flags = false;

        if self.enable_compression {
            write!(f, "c")?;
            has_flags = true;
        }

        if self.use_mock_prover {
            write!(f, "m")?;
            has_flags = true;
        }

        if self.stark_to_snark_conversion {
            write!(f, "s")?;
            has_flags = true;
        }

        if has_flags {
            write!(f, "_")?;
        }

        Ok(())
    }
}

impl Default for ProverOptions {
    fn default() -> Self {
        Self {
            enable_compression: false,
            use_mock_prover: true,
            stark_to_snark_conversion: false,
            use_cached_keys: true,
            proof_type: ProofType::Core,
        }
    }
}
