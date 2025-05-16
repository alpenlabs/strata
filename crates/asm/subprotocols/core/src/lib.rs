//! This module implements the “CoreASM” subprotocol, responsible for
//! on-chain verification and anchoring of zk-SNARK checkpoint proofs.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_asm_common::{ASMError, InterProtoMsg, SectionState, Subprotocol};
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

    /// Public key of the sequencer authorized to submit checkpoint proofs.
    sequencer_pubkey: Buf32,
}

impl Subprotocol for CoreASMState {
    fn id(&self) -> u8 {
        CORE_SUBPROTOCOL_ID
    }

    fn from_section(section: &SectionState) -> Result<Box<dyn Subprotocol>, ASMError>
    where
        Self: Sized,
    {
        let state = CoreASMState::try_from(section)?;
        Ok(Box::new(state))
    }

    fn finalize_state(&mut self, _msgs: &[InterProtoMsg]) -> (SectionState, Buf32) {
        let section: SectionState = self.clone().into();
        (section, Buf32::zero())
    }
}

impl From<CoreASMState> for SectionState {
    fn from(state: CoreASMState) -> Self {
        let data = borsh::to_vec(&state).expect("BorshSerialize on CoreASMState should never fail");
        SectionState {
            subprotocol_id: CORE_SUBPROTOCOL_ID,
            data,
        }
    }
}

impl TryFrom<&SectionState> for CoreASMState {
    type Error = ASMError;

    fn try_from(section: &SectionState) -> Result<Self, Self::Error> {
        if section.subprotocol_id != CORE_SUBPROTOCOL_ID {
            return Err(ASMError::InvalidSubprotocol(section.subprotocol_id));
        }
        CoreASMState::try_from_slice(&section.data)
            .map_err(|e| ASMError::Deserialization(section.subprotocol_id, e))
    }
}
