use borsh::{BorshDeserialize, BorshSerialize};
use strata_asm_common::{Subprotocol, error::ASMError, msg::InterProtoMsg, state::SectionState};
use strata_primitives::buf::Buf32;

pub const BRIDGE_V1_SUBPROTOCOL_ID: u8 = 2;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BridgeV1StateStub {}

impl Subprotocol for BridgeV1StateStub {
    fn id(&self) -> u8 {
        BRIDGE_V1_SUBPROTOCOL_ID
    }

    fn from_section(section: SectionState) -> Result<Box<dyn Subprotocol>, ASMError>
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
            subprotocol_id: BRIDGE_V1_SUBPROTOCOL_ID,
            data,
        }
    }
}

// 3) Parse the wire format back into your struct:
impl TryFrom<SectionState> for BridgeV1StateStub {
    type Error = ASMError;

    fn try_from(section: SectionState) -> Result<Self, Self::Error> {
        if section.subprotocol_id != BRIDGE_V1_SUBPROTOCOL_ID {
            return Err(ASMError::InvalidSubprotocol(section.subprotocol_id));
        }
        BridgeV1StateStub::try_from_slice(&section.data)
            .map_err(|e| ASMError::Deserialization(section.subprotocol_id, e))
    }
}
