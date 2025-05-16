//! BridgeV1 Subprotocol
use borsh::{BorshDeserialize, BorshSerialize};
use strata_asm_common::{ASMError, InterProtoMsg, SectionState, Subprotocol, SubprotocolId};
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

    fn from_section(section: &SectionState) -> Result<Box<dyn Subprotocol>, ASMError>
    where
        Self: Sized,
    {
        let state = BridgeV1StateStub::try_from(section)?;
        Ok(Box::new(state))
    }

    fn finalize_state(&mut self, _msgs: &[InterProtoMsg]) -> (SectionState, Buf32) {
        let section: SectionState = self.clone().into();
        (section, Buf32::zero())
    }
}

impl From<BridgeV1StateStub> for SectionState {
    fn from(state: BridgeV1StateStub) -> Self {
        let data =
            borsh::to_vec(&state).expect("BorshSerialize on BridgeV1StateStub should never fail");
        SectionState {
            id: BRIDGE_V1_SUBPROTOCOL_ID,
            data,
        }
    }
}

impl TryFrom<&SectionState> for BridgeV1StateStub {
    type Error = ASMError;

    fn try_from(section: &SectionState) -> Result<Self, Self::Error> {
        if section.id != BRIDGE_V1_SUBPROTOCOL_ID {
            return Err(ASMError::InvalidSubprotocol(section.id));
        }
        BridgeV1StateStub::try_from_slice(&section.data)
            .map_err(|e| ASMError::Deserialization(section.id, e))
    }
}
