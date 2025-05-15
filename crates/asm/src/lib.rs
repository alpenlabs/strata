//! Core types and state transition logic for the Anchor State Machine (ASM).
//!
//! The ASM anchors the Strata orchestration layer to L1, analogous to a rollup smart contract.

use std::collections::HashMap;

use bitcoin::{block::Block, params::Params};
use strata_primitives::{
    buf::Buf32,
    l1::{HeaderVerificationState, L1BlockId},
};
use subprotocol::{Subprotocol, core::CoreASMState};
use thiserror::Error;
mod msg;
mod subprotocol;

#[derive(Debug, Clone)]
pub struct AnchorState {
    chain_view: ChainViewState,

    /// This needs to be sorted by Subprotocol Version/ID
    sections: Vec<SectionState>,
}

impl AnchorState {
    /// Constructs a Vec of all successfully deserialized subprotocol instances
    pub fn subprotocols(&self) -> Vec<impl Subprotocol> {
        let mut protocols = Vec::with_capacity(self.sections.len());
        for section in &self.sections {
            // Ignore any sections that fail to deserialize
            if let Ok(proto) = section.subprotocol() {
                protocols.push(proto);
            }
        }
        protocols
    }
}

#[derive(Debug, Clone)]
pub struct ChainViewState {
    pub pow_state: HeaderVerificationState,
    pub events: Vec<(L1BlockId, Vec<(u8, Buf32)>)>,
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
    let mut protocols = pre_state.subprotocols();
    let mut pow_state = pre_state.chain_view.pow_state.clone();
    let mut events = pre_state.chain_view.events.clone();

    pow_state
        .check_and_update_continuity(&block.header, &Params::MAINNET)
        .expect("header doesn't follow the consensus rules");

    let mut inter_msgs = HashMap::new();
    for protocol in protocols.iter_mut() {
        let msgs = protocol.process_block(&block);
        for (id, msg) in msgs {
            inter_msgs.entry(id).or_insert_with(Vec::new).push(msg);
        }
    }

    let mut mmr_events = Vec::new();
    let mut sections = Vec::new();
    for protocol in protocols.iter_mut() {
        let id = protocol.id();
        let msgs = inter_msgs.entry(id).or_default();
        let (section, mmr_event_hash) = protocol.finalize_state(msgs);
        sections.push(section);
        mmr_events.push((id, mmr_event_hash));
    }

    events.push((*pow_state.last_verified_block.blkid(), mmr_events));

    let chain_view = ChainViewState { pow_state, events };

    AnchorState {
        chain_view,
        sections,
    }
}
