use bitcoin::Block;
use serde::{Deserialize, Serialize};
use strata_tx_parser::filter::TxFilterRule;

use super::vms::ProofVm;
use crate::proving_ops::{
    checkpoint_ops::CheckpointInput, cl_ops::CLProverInput, l1_batch_ops::L1BatchInput,
    l2_batch_ops::L2BatchInput,
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
#[allow(clippy::large_enum_variant)]
pub enum ProverInput {
    BtcBlock(Block, Vec<TxFilterRule>),
    ElBlock(WitnessData),
    ClBlock(CLProverInput),
    L1Batch(L1BatchInput),
    L2Batch(L2BatchInput),
    Checkpoint(CheckpointInput),
}

impl ProverInput {
    pub fn proof_vm_id(&self) -> ProofVm {
        match self {
            ProverInput::BtcBlock(_, _) => ProofVm::BtcProving,
            ProverInput::ElBlock(_) => ProofVm::ELProving,
            ProverInput::ClBlock(_) => ProofVm::CLProving,
            ProverInput::L1Batch(_) => ProofVm::L1Batch,
            ProverInput::L2Batch(_) => ProofVm::CLAggregation,
            ProverInput::Checkpoint(_) => ProofVm::Checkpoint,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WitnessData {
    pub data: Vec<u8>,
}
