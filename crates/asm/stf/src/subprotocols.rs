use strata_asm_common::{SectionState, Subprotocol};
use strata_asm_proto_bridge_v1::{BRIDGE_V1_SUBPROTOCOL_ID, BridgeV1State, BridgeV1Subproto};
use strata_asm_proto_core::{CORE_SUBPROTOCOL_ID, CoreASMState, OLCoreSubproto};

// TODO remove this, I think it's redundant with the new SubprotoHandler concept
/*
/// Parse all of the `sections` into a `Vec<Box<dyn Subprotocol>>`.
/// Unknown protocols are simply skipped.
pub(crate) fn parse_subprotocols(sections: &[SectionState]) -> Vec<Box<dyn Subprotocol>> {
    sections
        .iter()
        .filter_map(|sec| match sec.id {
            CORE_SUBPROTOCOL_ID => CoreASMState::try_from_section(sec).ok(),
            BRIDGE_V1_SUBPROTOCOL_ID => BridgeV1StateStub::try_from_section(sec).ok(),
            _ => None,
        })
        .collect()
}
*/
