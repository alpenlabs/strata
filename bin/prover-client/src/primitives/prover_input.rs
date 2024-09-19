use serde::{Deserialize, Serialize};

use super::vms::ProofVm;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProverInput {
    ElBlock(WitnessData),
    ClBlock(WitnessData),
}

impl ProverInput {
    pub fn proof_vm_id(&self) -> ProofVm {
        match self {
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
