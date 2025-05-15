use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{batch::EpochSummary, buf::Buf32, l1::L1BlockId};
use zkaleido::VerifyingKey;

use super::Subprotocol;
use crate::{ASMError, SectionState};

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct CoreSubprotocol;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct CoreASMState {
    /// VerifyingKey to verify the checkpoint that has been posted onto Bitcoin
    checkpoint_vk: VerifyingKey,
    /// State of the checkpoint that has been last verified
    /// The new checkpoint proof will be verified against this state
    verified_checkpoint: EpochSummary,
    /// L1BlockId till where the last checkpoint covers
    last_checkpoint_ref: L1BlockId,
    /// PulicKey of the batch producer
    batch_producer_pubkey: Buf32,
    /// Administrator of the subprotocol. Administrator is able to change the batch producer pubkey
    /// via a Bitcoin Transaction
    administrator: Buf32,
    /// Consusus Manager
    consensus_manager: Buf32,
}

impl Subprotocol for CoreASMState {
    const VERSION: u8 = 1;

    fn to_section_state(&self) -> SectionState {
        let data = borsh::to_vec(&self).expect("serialization of subprotocol failed");
        SectionState {
            subprotocol_id: 1, // FIXME:
            data,
        }
    }
}

impl TryFrom<&SectionState> for CoreASMState {
    type Error = ASMError;

    fn try_from(section: &SectionState) -> Result<Self, Self::Error> {
        CoreASMState::try_from_slice(&section.data)
            .map_err(|e| ASMError::Deserialization(section.id(), e))
    }
}

impl From<CoreASMState> for SectionState {
    fn from(value: CoreASMState) -> Self {
        let data = borsh::to_vec(&value).expect("serialization of subprotocol failed");
        SectionState {
            subprotocol_id: 1, // FIXME:
            data,
        }
    }
}
