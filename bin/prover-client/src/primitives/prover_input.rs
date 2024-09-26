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
            ProverInput::BtcBlock(..) => ProofVm::BtcBlockspaceProving,
            ProverInput::ElBlock(_) => ProofVm::ELProving,
            ProverInput::ClBlock(_) => ProofVm::CLProving,
        }
    }
}

impl Default for ProverInput {
    fn default() -> Self {
        ProverInput::ElBlock(WitnessData::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WitnessData {
    pub data: Vec<u8>,
}
