use strata_proofimpl_cl_stf::L2BatchProofOutput;
use zkaleido::{
    AggregationInput, ProofReceipt, PublicValues, VerificationKey, ZkVmInputResult, ZkVmProver,
    ZkVmResult,
};

pub struct ClAggInput {
    pub batch: Vec<ProofReceipt>,
    pub cl_stf_vk: VerificationKey,
}

pub struct ClAggProver;

impl ZkVmProver for ClAggProver {
    type Input = ClAggInput;
    type Output = L2BatchProofOutput;

    fn name() -> String {
        "CL Agg".to_string()
    }

    fn proof_type() -> zkaleido::ProofType {
        zkaleido::ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: zkaleido::ZkVmInputBuilder<'a>,
    {
        let len = input.batch.len() as u32;
        let mut input_builder = B::new();
        input_builder.write_serde(&len)?;

        for proof in &input.batch {
            input_builder.write_proof(&AggregationInput::new(
                proof.clone(),
                input.cl_stf_vk.clone(),
            ))?;
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
