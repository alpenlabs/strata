use bitcoin::Block;
use serde::{Deserialize, Serialize};
use strata_tx_parser::filter::TxFilterRule;

use super::vms::ProofVm;

#[derive(Debug, Clone)]
pub enum ProverInput {
    BtcBlock(Block, Vec<TxFilterRule>),
    ElBlock(WitnessData),
    ClBlock(WitnessData),
}

impl ProverInput {
    pub fn proof_vm_id(&self) -> ProofVm {
        match self {
            ProverInput::BtcBlock(_, _) => ProofVm::BtcProving,
            ProverInput::ElBlock(_) => ProofVm::ELProving,
            ProverInput::ClBlock(_) => ProofVm::CLProving,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WitnessData {
    pub data: Vec<u8>,
}
