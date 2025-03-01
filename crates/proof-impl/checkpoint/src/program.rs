use std::sync::Arc;

use strata_primitives::buf::Buf32;
use zkaleido::{
    AggregationInput, ProofReceipt, PublicValues, VerifyingKey, ZkVmInputResult, ZkVmProgram,
    ZkVmProgramPerf, ZkVmResult,
};
use zkaleido_native_adapter::{NativeHost, NativeMachine};

use crate::process_checkpoint_proof_outer;

pub struct CheckpointProverInput {
    pub cl_stf_proofs: Vec<ProofReceipt>,
    pub cl_stf_vk: VerifyingKey,
}

pub struct CheckpointProgram;

impl ZkVmProgram for CheckpointProgram {
    type Input = CheckpointProverInput;
    type Output = (Buf32, Buf32);

    fn name() -> String {
        "Checkpoint".to_string()
    }

    fn proof_type() -> zkaleido::ProofType {
        zkaleido::ProofType::Groth16
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: zkaleido::ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();

        input_builder.write_serde(&input.cl_stf_proofs.len())?;

        for cl_stf_proof in &input.cl_stf_proofs {
            let cl_stf_proof_with_vk =
                AggregationInput::new(cl_stf_proof.clone(), input.cl_stf_vk.clone());
            input_builder.write_proof(&cl_stf_proof_with_vk)?;
        }

        input_builder.build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: zkaleido::ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}

impl ZkVmProgramPerf for CheckpointProgram {}

impl CheckpointProgram {
    pub fn native_host() -> NativeHost {
        const MOCK_VK: [u32; 8] = [0u32; 8];
        NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_checkpoint_proof_outer(zkvm, &MOCK_VK);
                Ok(())
            })),
        }
    }

    // Add this new convenience method
    pub fn execute(
        input: &<Self as ZkVmProgram>::Input,
    ) -> ZkVmResult<<Self as ZkVmProgram>::Output> {
        // Get the native host and delegate to the trait's execute method
        let host = Self::native_host();
        <Self as ZkVmProgram>::execute(input, &host)
    }
}
