use strata_primitives::params::RollupParams;
use strata_state::{block::L2Block, chain_state::ChainState};
use strata_zkvm::{AggregationInput, Proof, VerificationKey, ZkVmProver};

use crate::L2BatchProofOutput;

pub struct ClStfInput {
    pub rollup_params: RollupParams,
    pub pre_state: ChainState,
    pub l2_block: L2Block,
    pub evm_ee_proof: Proof,
    pub evm_ee_vk: VerificationKey,
}

pub struct ClStfProver;

impl ZkVmProver for ClStfProver {
    type Input = ClStfInput;
    type Output = L2BatchProofOutput;

    fn proof_type() -> strata_zkvm::ProofType {
        strata_zkvm::ProofType::Compressed
    }

    fn proof_name() -> String {
        "CL STF".to_string()
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> anyhow::Result<B::Input>
    where
        B: strata_zkvm::ZkVmInputBuilder<'a>,
    {
        B::new()
            .write_serde(&input.rollup_params)?
            .write_borsh(&(&input.pre_state, &input.l2_block))?
            .write_proof(AggregationInput::new(
                input.evm_ee_proof.clone(),
                input.evm_ee_vk.clone(),
            ))?
            .build()
    }

    fn process_output<H>(proof: &Proof, _host: &H) -> anyhow::Result<Self::Output>
    where
        H: strata_zkvm::ZkVmHost,
    {
        H::extract_borsh_public_output(proof)
    }
}
