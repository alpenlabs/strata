use std::hash::Hash;

use strata_primitives::proof::ProofId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofVm {
    BtcProving,
    ELProving,
    CLProving,
    CLAggregation,
    L1Batch,
    Checkpoint,
}

impl From<ProofId> for ProofVm {
    fn from(value: ProofId) -> Self {
        match value {
            ProofId::BtcBlockspace(_) => Self::BtcProving,
            ProofId::L1Batch(_, _) => Self::L1Batch,
            ProofId::EvmEeStf(_) => Self::ELProving,
            ProofId::ClStf(_) => Self::CLProving,
            ProofId::ClAgg(_, _) => Self::CLAggregation,
            ProofId::Checkpoint(_) => Self::Checkpoint,
        }
    }
}
