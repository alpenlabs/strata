use strata_primitives::{
    buf::Buf32,
    l1::{HeaderVerificationState, L1BlockId},
};

use crate::{
    core::CORE_SUBPROTOCOL_ID,
    error::ASMError,
    subprotocol::{Subprotocol, core::CoreASMState},
};

/// Anchor state for the Anchor State Machine (ASM), the core of the Strata protocol.
///
/// The ASM anchors the orchestration layer to L1, akin to a host smart contract
/// in an EVM environment. It defines a pure state transition function (STF)
/// over L1 blocks: given a prior ASM state and a new L1 block, it computes the
/// next ASM state off-chain. Conceptually, this is like a stateful smart contract
/// receiving protocol transactions at L1 and updating its storage. A zk-SNARK proof
/// attests that the transition from the previous ASM state to the new state
/// was performed correctly on the given L1 block.
#[derive(Debug, Clone)]
pub struct AnchorState {
    /// The current view of the L1 chain required for state transitions.
    pub chain_view: ChainViewState,

    /// States for each subprotocol section, sorted by Subprotocol Version/ID.
    pub sections: Vec<SectionState>,
}

/// Represents the on‐chain view required by the Anchor State Machine (ASM) to process
/// state transitions for each new L1 block.
#[derive(Debug, Clone)]
pub struct ChainViewState {
    /// All data needed to validate a Bitcoin block header, including past‐n timestamps,
    /// accumulated work, and difficulty adjustments.
    pub pow_state: HeaderVerificationState,

    /// Events emitted by subprotocols, keyed by L1 block:
    /// - The outer Vec is ordered by block ID.
    /// - Each inner Vec contains tuples of `(subprotocol_id, event_hash)`.
    ///
    /// `subprotocol_id` (u8) identifies which subprotocol emitted the event,
    /// and `Buf32` is the protocol‐computed hash of the event payload.
    // TODO: Eventually this will use an MMR for minimal, non‐linear‐growing on‐chain state.
    pub events: Vec<(L1BlockId, Vec<(u8, Buf32)>)>,
}

/// Holds the off‐chain serialized state for a single subprotocol section within the ASM.
///
/// Each `SectionState` pairs the subprotocol’s unique ID with its current serialized state,
/// allowing the ASM to apply the appropriate state transition logic for that subprotocol.
#[derive(Debug, Clone)]
pub struct SectionState {
    /// Identifier of the subprotocol
    pub subprotocol_id: u8,

    /// The serialized subprotocol state.
    ///
    /// This is normally fairly small, but we are setting a comfortable max limit.
    pub data: Vec<u8>,
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

impl SectionState {
    /// Returns the identifier of the subprotocol for this section.
    ///
    /// This ID corresponds to the version or namespace of the subprotocol whose
    /// state is serialized in this section.
    pub fn id(&self) -> u8 {
        self.subprotocol_id
    }

    /// Attempts to deserialize this section into its corresponding subprotocol state machine.
    ///
    /// Uses the section’s `subprotocol_id` to select and construct the appropriate
    /// `Subprotocol` implementation via `TryFrom<SectionState>`.
    pub fn subprotocol(&self) -> Result<impl Subprotocol, ASMError> {
        match self.id() {
            CORE_SUBPROTOCOL_ID => CoreASMState::try_from(self),
            _ => Err(ASMError::InvalidSubprotocol(self.id())),
        }
    }
}
