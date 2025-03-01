use strata_primitives::buf::Buf32;
use zkaleido::{
    AggregationInput, ProofReceipt, PublicValues, VerifyingKey, ZkVmInputResult, ZkVmProgram,
    ZkVmProgramPerf, ZkVmResult,
};

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
