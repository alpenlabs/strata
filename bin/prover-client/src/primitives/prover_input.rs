use bitcoin::Block;
use serde::{Deserialize, Serialize};
use strata_primitives::params::RollupParams;
use strata_zkvm::AggregationInput;

use super::vms::ProofVm;
use crate::proving_ops::{
    checkpoint_ops::CheckpointInput, cl_ops::CLProverInput, l1_batch_ops::L1BatchInput,
    l2_batch_ops::L2BatchInput,
};

pub type ProofWithVkey = AggregationInput;

#[derive(Debug, Clone)]
pub enum ZKVMInput {
    BtcBlock(Block, RollupParams),
    ElBlock(WitnessData),
    ClBlock(CLProverInput),
    L1Batch(L1BatchInput),
    L2Batch(L2BatchInput),
    Checkpoint(CheckpointInput),
}

impl ZKVMInput {
    pub fn proof_vm_id(&self) -> ProofVm {
        match self {
            ZKVMInput::BtcBlock(_, _) => ProofVm::BtcProving,
            ZKVMInput::ElBlock(_) => ProofVm::ELProving,
            ZKVMInput::ClBlock(_) => ProofVm::CLProving,
            ZKVMInput::L1Batch(_) => ProofVm::L1Batch,
            ZKVMInput::L2Batch(_) => ProofVm::CLAggregation,
            ZKVMInput::Checkpoint(_) => ProofVm::Checkpoint,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WitnessData {
    pub data: Vec<u8>,
}
