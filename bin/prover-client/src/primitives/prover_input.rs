use bitcoin::Block;
use serde::{Deserialize, Serialize};
use strata_tx_parser::filter::TxFilterRule;

use super::vms::ProofVm;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProverInput {
    BtcBlock(Block, Vec<TxFilterRule>),
    ElBlock(WitnessData),
    ClBlock(WitnessData),
    L1Batch(WitnessData),
}

impl ProverInput {
    pub fn proof_vm_id(&self) -> ProofVm {
        match self {
            ProverInput::BtcBlock(_, _) => ProofVm::BtcProving,
            ProverInput::ElBlock(_) => ProofVm::ELProving,
            ProverInput::ClBlock(_) => ProofVm::CLProving,
            ProverInput::L1Batch(_) => ProofVm::L1Batch,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WitnessData {
    pub data: Vec<u8>,
}
