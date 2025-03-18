use std::sync::Arc;

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_state::{
    batch::{ChainstateRootTransition, TxFilterConfigTransition},
    block::L2Block,
    chain_state::Chainstate,
};
use zkaleido::{
    AggregationInput, ProofReceipt, PublicValues, VerifyingKey, ZkVmInputResult, ZkVmProgram,
    ZkVmProgramPerf, ZkVmResult,
};
use zkaleido_native_adapter::{NativeHost, NativeMachine};

use crate::process_cl_stf;

/// Input to the chain state transition function (STF) for the Consensus Layer (CL).
///
/// This structure encapsulates all the data required to update the chain state
/// by applying a sequence of L2 blocks, along with the corresponding proofs and
/// verifying keys for both the EVM execution environment and BTC blockspace.
pub struct ClStfInput {
    /// Rollup parameters used to configure the state transition.
    pub rollup_params: RollupParams,

    /// The chain state prior to applying any L2 blocks.
    pub chainstate: Chainstate,

    /// A list of L2 blocks to be applied to the chain state.
    /// The chain state is modified sequentially by these blocks.
    pub l2_blocks: Vec<L2Block>,

    /// A tuple containing the EVM execution environment proof receipt and its
    /// corresponding verifying key.
    /// Each L2 block has an associated EVM execution segment.
    pub evm_ee_proof_with_vk: (ProofReceipt, VerifyingKey),

    /// An optional tuple containing the BTC blockspace proof receipt and its
    /// corresponding verifying key.
    /// Each L2 block may include an optional L1 segment; this segment is present
    /// only for the terminal L2 block of an epoch. If the provided L2 blocks include
    /// the terminal block, this field contains a value; otherwise, it is set to None.
    pub btc_blockspace_proof_with_vk: Option<(ProofReceipt, VerifyingKey)>,
}

/// Output from the Consensus Layer State Transition Function (CL STF).
///
/// This structure represents the result of applying a sequence of L2 blocks
/// to the chain state. It includes the epoch identifier, the chain state roots
/// before and after the transition, and an optional transaction filters transition
/// if a terminal block is present.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ClStfOutput {
    /// The epoch during which the provided L2 blocks were processed.
    pub epoch: u64,
    /// The chain state root prior to applying the L2 blocks.
    pub initial_chainstate_root: Buf32,
    /// The chain state root after applying the L2 blocks.
    pub final_chainstate_root: Buf32,
    /// An optional transition for the transaction filter configuration.
    /// This is populated only if the terminal block of the epoch is included in the input;
    /// otherwise, it is None.
    pub tx_filters_transition: Option<TxFilterConfigTransition>,
    /// An optional previous chainstate transition.
    /// This field is present if a checkpoint transaction corresponding to the previous epoch is
    /// found in one of the L1 segments. In such cases, the chain state root at the beginning
    /// of the current epoch must equal the chain state root at the end of the previous epoch.
    ///
    /// NOTE: This check is done in the checkpoint proof since we might need to break epoch into
    /// multiple CL STF proofs.
    pub prev_chainstate_transition: Option<ChainstateRootTransition>,
}

pub struct ClStfProgram;

impl ZkVmProgram for ClStfProgram {
    type Input = ClStfInput;
    type Output = ClStfOutput;

    fn name() -> String {
        "CL STF".to_string()
    }

    fn proof_type() -> zkaleido::ProofType {
        zkaleido::ProofType::Compressed
    }

    fn prepare_input<'a, B>(input: &'a Self::Input) -> ZkVmInputResult<B::Input>
    where
        B: zkaleido::ZkVmInputBuilder<'a>,
    {
        let mut input_builder = B::new();
        input_builder.write_serde(&input.rollup_params)?;
        input_builder.write_borsh(&input.chainstate)?;
        input_builder.write_borsh(&input.l2_blocks)?;

        match input.btc_blockspace_proof_with_vk.clone() {
            Some((proof, vk)) => {
                input_builder.write_serde(&true)?;
                input_builder.write_proof(&AggregationInput::new(proof, vk))?;
            }
            None => {
                input_builder.write_serde(&false)?;
            }
        };

        let (proof, vk) = input.evm_ee_proof_with_vk.clone();
        input_builder.write_proof(&AggregationInput::new(proof, vk))?;

        input_builder.build()
    }

    fn process_output<H>(public_values: &PublicValues) -> ZkVmResult<Self::Output>
    where
        H: zkaleido::ZkVmHost,
    {
        H::extract_borsh_public_output(public_values)
    }
}

impl ZkVmProgramPerf for ClStfProgram {}

impl ClStfProgram {
    pub fn native_host() -> NativeHost {
        const MOCK_VK: [u32; 8] = [0u32; 8];
        NativeHost {
            process_proof: Arc::new(Box::new(move |zkvm: &NativeMachine| {
                process_cl_stf(zkvm, &MOCK_VK, &MOCK_VK);
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
