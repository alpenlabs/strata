use strata_asm_common::{Subprotocol, msg::InterProtoMsg, state::SectionState};
use strata_primitives::buf::Buf32;

pub const BRIDGE_V1_SUBPROTOCOL_ID: u8 = 2;

pub struct BridgeV1StateStub {}

impl Subprotocol for BridgeV1StateStub {
    const VERSION: u8 = BRIDGE_V1_SUBPROTOCOL_ID;

    fn finalize_state(&mut self, _msgs: &[InterProtoMsg]) -> (SectionState, Buf32) {
        (
            SectionState {
                subprotocol_id: BRIDGE_V1_SUBPROTOCOL_ID,
                data: Vec::new(),
            },
            Buf32::zero(),
        )
    }
}
