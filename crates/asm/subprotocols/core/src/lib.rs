use borsh::{BorshDeserialize, BorshSerialize};
use strata_asm_common::{Subprotocol, error::ASMError, msg::InterProtoMsg, state::SectionState};
use strata_primitives::{batch::EpochSummary, buf::Buf32, l1::L1BlockId};
use zkaleido::VerifyingKey;

/// The unique identifier for the CoreASM subprotocol within the Anchor State Machine.
///
/// This constant is used to tag `SectionState` entries belonging to the CoreASM logic
/// and must match the `subprotocol_id` checked in `SectionState::subprotocol()`.
pub const CORE_SUBPROTOCOL_ID: u8 = 1;

/// State for the CoreASM subprotocol, responsible for validating outgoing-layer (OL)
/// checkpoints posted onto Bitcoin.
///
/// The CoreASM subprotocol ensures that each zk‐SNARK proof of a new checkpoint
/// is correctly verified against the last known checkpoint state anchored on L1.
/// It manages the verifying key, tracks the latest verified checkpoint, and
/// enforces administrative controls over batch producer and consensus manager keys.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct CoreASMState {
    /// The zk‐SNARK verifying key used to verify each new checkpoint proof
    /// that has been posted on Bitcoin.
    checkpoint_vk: VerifyingKey,

    /// Summary of the last checkpoint that was successfully verified.
    /// New proofs are checked against this epoch summary.
    verified_checkpoint: EpochSummary,

    /// The L1 block ID up to which the `verified_checkpoint` covers.
    last_checkpoint_ref: L1BlockId,

    /// Public key of the batch producer authorized to submit checkpoint proofs.
    batch_producer_pubkey: Buf32,

    /// Administrator public key. Holds the authority to update the
    /// `batch_producer_pubkey` via a Bitcoin transaction.
    administrator: Buf32,

    /// Public key of the consensus manager, responsible for higher‐level
    /// governance and protocol parameter updates.
    consensus_manager: Buf32,
}

impl CoreASMState {
    fn to_section_state(&self) -> SectionState {
        let data = borsh::to_vec(&self).expect("serialization of subprotocol failed");
        SectionState {
            subprotocol_id: CORE_SUBPROTOCOL_ID,
            data,
        }
    }
}

impl Subprotocol for CoreASMState {
    const VERSION: u8 = CORE_SUBPROTOCOL_ID;

    fn finalize_state(&mut self, _msgs: &[InterProtoMsg]) -> (SectionState, Buf32) {
        (self.to_section_state(), Buf32::zero())
    }
}

impl TryFrom<&SectionState> for CoreASMState {
    type Error = ASMError;

    fn try_from(section: &SectionState) -> Result<Self, Self::Error> {
        CoreASMState::try_from_slice(&section.data)
            .map_err(|e| ASMError::Deserialization(CORE_SUBPROTOCOL_ID, e))
    }
}
