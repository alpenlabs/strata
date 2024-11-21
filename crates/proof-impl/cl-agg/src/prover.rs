use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_zkvm::{AggregationInput, Proof, VerificationKey, ZkVmProver};

pub struct ClAggInput {
    pub batch: Vec<Proof>,
    pub cl_stf_vk: VerificationKey,
}

pub struct ClAggProver;

impl ZkVmProver for ClAggProver {
    type Input = ClAggInput;
    type Output = L2BatchProofOutput;

    fn proof_type() -> strata_zkvm::ProofType {
        strata_zkvm::ProofType::Compressed
    }

    fn proof_name() -> String {
        "CL Agg".to_owned()
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> anyhow::Result<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
    {
        let len = input.batch.len() as u32;
        let mut input_builder = B::new();
        input_builder.write_serde(&len)?;

        for proof in &input.batch {
            input_builder.write_proof(AggregationInput::new(
                proof.clone(),
                input.cl_stf_vk.clone(),
            ))?;
        }

        input_builder.build()
    }

    fn process_output<H>(proof: &Proof, _host: &H) -> anyhow::Result<Self::Output>
    where
        H: strata_zkvm::ZkVmHost,
    {
        H::extract_borsh_public_output(proof)
    }
}
