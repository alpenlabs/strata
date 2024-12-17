use bitcoin::Block;
use serde::{Deserialize, Serialize};
use strata_primitives::params::RollupParams;
use strata_zkvm::AggregationInput;
use strata_zkvm_hosts::ProofVm;

use crate::proving_ops::{
    checkpoint_ops::CheckpointInput, cl_ops::CLProverInput, l1_batch_ops::L1BatchInput,
    l2_batch_ops::L2BatchInput,
};

pub type ProofWithVkey = AggregationInput;

#[derive(Debug, Clone)]
pub enum ZkVmInput {
    BtcBlock(Block, RollupParams),
    ElBlock(WitnessData),
    ClBlock(CLProverInput),
    L1Batch(L1BatchInput),
    L2Batch(L2BatchInput),
    Checkpoint(CheckpointInput),
}

impl ZkVmInput {
    pub fn proof_vm_id(&self) -> ProofVm {
        match self {
            ZkVmInput::BtcBlock(_, _) => ProofVm::BtcProving,
            ZkVmInput::ElBlock(_) => ProofVm::ELProving,
            ZkVmInput::ClBlock(_) => ProofVm::CLProving,
            ZkVmInput::L1Batch(_) => ProofVm::L1Batch,
            ZkVmInput::L2Batch(_) => ProofVm::CLAggregation,
            ZkVmInput::Checkpoint(_) => ProofVm::Checkpoint,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WitnessData {
    pub data: Vec<u8>,
}
