//! BridgeV1 Subprotocol
use borsh::{BorshDeserialize, BorshSerialize};
use strata_asm_common::{InterProtoMsg, SectionState, Subprotocol, SubprotocolId};
use strata_primitives::buf::Buf32;

/// The unique identifier for the BridgeV1 subprotocol within the Anchor State Machine.
///
/// This constant is used to tag `SectionState` entries belonging to the CoreASM logic
/// and must match the `subprotocol_id` checked in `SectionState::subprotocol()`.
pub const BRIDGE_V1_SUBPROTOCOL_ID: SubprotocolId = 2;

/// A minimal stub
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BridgeV1StateStub {}

impl Subprotocol for BridgeV1StateStub {
    fn id(&self) -> SubprotocolId {
        BRIDGE_V1_SUBPROTOCOL_ID
    }

    fn finalize_state(&mut self, _msgs: &[InterProtoMsg]) -> (SectionState, Buf32) {
        let section = self.to_section();
        (section, Buf32::zero())
    }
}
