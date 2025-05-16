use strata_asm_common::{Subprotocol, state::SectionState};
use strata_asm_proto_bridge_v1::{BRIDGE_V1_SUBPROTOCOL_ID, BridgeV1StateStub};
use strata_asm_proto_core::{CORE_SUBPROTOCOL_ID, CoreASMState};

pub fn parse_subprotocols(sections: &[SectionState]) -> Vec<Box<dyn Subprotocol>> {
    sections
        .iter()
        .filter_map(|sec| match sec.subprotocol_id {
            CORE_SUBPROTOCOL_ID => CoreASMState::from_section(sec.clone()).ok(),
            BRIDGE_V1_SUBPROTOCOL_ID => BridgeV1StateStub::from_section(sec.clone()).ok(),
            _ => None,
        })
        .collect()
}
