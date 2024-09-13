use alpen_express_db::types::WitnessType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProverInput {
    ElBlock(WitnessData),
    ClBlock(WitnessData),
}

impl ProverInput {
    pub fn proof_vm_id(&self) -> WitnessType {
        match self {
            ProverInput::ElBlock(_) => WitnessType::EL,
            ProverInput::ClBlock(_) => WitnessType::CL,
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        match self {
            ProverInput::ElBlock(witness) => witness.data.clone(),
            ProverInput::ClBlock(witness) => witness.data.clone(),
        }
    }

    pub fn make_empty(&mut self) {
        match self {
            ProverInput::ElBlock(witness) => witness.data.clear(),
            ProverInput::ClBlock(witness) => witness.data.clear(),
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
