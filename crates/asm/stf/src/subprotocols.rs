use strata_asm_common::{Subprotocol, error::ASMError, state::SectionState};
use strata_asm_proto_core::CoreASMState;

pub const CORE_SUBPROTOCOL_ID: u8 = 1;

pub fn parse_subprotocols(sections: &[SectionState]) -> Vec<impl Subprotocol> {
    let mut protocols = Vec::with_capacity(sections.len());
    for section in sections {
        // Ignore any sections that fail to deserialize
        if let Ok(proto) = try_parse_subprotocol(section) {
            protocols.push(proto);
        }
    }
    protocols
}

pub fn try_parse_subprotocol(section: &SectionState) -> Result<impl Subprotocol, ASMError> {
    match section.subprotocol_id {
        CORE_SUBPROTOCOL_ID => CoreASMState::try_from(section),
        _ => Err(ASMError::InvalidSubprotocol(section.subprotocol_id)),
    }
}
