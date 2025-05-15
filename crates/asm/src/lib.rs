//! Core types and state transition logic for the Anchor State Machine (ASM).
//!
//! The ASM anchors the Strata orchestration layer to L1, analogous to a rollup smart contract.

use std::collections::HashMap;

use bitcoin::block::Block;
use msg::ProtoEvent;
use strata_primitives::l1::{HeaderVerificationState, L1BlockId};
use subprotocol::{Subprotocol, core::CoreASMState};
use thiserror::Error;
mod msg;
mod subprotocol;
mod tx_indexer;

#[derive(Debug, Clone)]
pub struct AnchorState {
    chain_view: ChainViewState,

    sections: Vec<SectionState>,
}

impl AnchorState {
    /// Constructs a map of all successfully deserialized subprotocol instances keyed by their ID.
    // TODO: Experiment with using BTreeMap
    pub fn subprotocols(&self) -> HashMap<u8, impl Subprotocol> {
        let mut map = HashMap::with_capacity(self.sections.len());
        for section in &self.sections {
            // Ignore any sections that fail to deserialize
            if let Ok(proto) = section.subprotocol() {
                map.insert(section.id(), proto);
            }
        }
        map
    }
}

#[derive(Debug, Clone)]
pub struct ChainViewState {
    pow_state: HeaderVerificationState,
    headers: Vec<(L1BlockId, Vec<ProtoEvent>)>,
}
#[derive(Debug, Clone)]
pub struct SectionState {
    /// Identifier of the subprotocol
    subprotocol_id: u8,

    /// The serialized subprotocol state.
    ///
    /// This is normally fairly small, but we are setting a comfortable max limit.
    data: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum ASMError {
    #[error("Subprotocol {0:?} do not exist")]
    InvalidSubprotocol(u8),

    #[error("Subprotocol {0:?} do not exist")]
    InvalidSubprotocolState(u8),

    #[error("Failed to deserialize subprotocol {0:?} state")]
    Deserialization(u8, #[source] borsh::io::Error),

    #[error("Failed to serialize subprotocol {0:?} state")]
    Serialization(u8, #[source] borsh::io::Error),
}

impl SectionState {
    pub fn id(&self) -> u8 {
        self.subprotocol_id
    }

    pub fn subprotocol(&self) -> Result<impl Subprotocol, ASMError> {
        match self.id() {
            1 => CoreASMState::try_from(self),
            _ => Err(ASMError::InvalidSubprotocol(self.id())),
        }
    }
}

pub fn asm_stf(pre_state: AnchorState, block: Block) -> AnchorState {
    let mut new_state = pre_state.clone();
    let mut protocols = pre_state.subprotocols();

    let mut inter_msgs = HashMap::new();
    for protocol in protocols.values_mut() {
        let msgs = protocol.process_block(&block);
        for (id, msg) in msgs {
            inter_msgs.entry(id).or_insert_with(Vec::new).push(msg);
        }
    }

    let mut protocol_events = Vec::new();
    for (id, msgs) in inter_msgs {
        let protocol = protocols.get_mut(&id).unwrap();
        let events = protocol.finalize_state(&msgs);
        protocol_events.push((id, events));
    }

    let mut sections: Vec<SectionState> = Vec::with_capacity(protocols.len());
    for (id, protocol) in protocols {
        sections.push(protocol.to_section_state());
    }

    new_state
}
