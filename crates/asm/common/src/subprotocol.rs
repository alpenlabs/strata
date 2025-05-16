//! Subprotocol trait definition for ASM.
//!
//! This trait defines the interface every ASM subprotocol implementation must
//! provide. Each subprotocol is responsible for parsing its transactions,
//! updating its internal state, and emitting cross-protocol messages and logs.

use bitcoin::Block;
use strata_primitives::buf::Buf32;

use crate::{error::ASMError, msg::InterProtoMsg, state::SectionState};

/// ASM subprotocol interface.
///
/// A Subprotocol encapsulates a self-contained piece of logic that
///
/// 1. processes each new L1 block to update its own state and emit outgoing inter-protocol
///    messages, and then
/// 2. receives incoming messages to finalize and serialize its state for inclusion in the global
///    AnchorState.
///
/// Each implementor must provide:
/// - A unique `VERSION: u8` constant (used as the `SectionState` tag).
/// - A `from_section` constructor to rehydrate from the wire format.
/// - The two core hooks: `process_block` and `finalize_state`.
pub trait Subprotocol {
    /// Reconstructs your subprotocol instance from its prior `SectionState`.
    ///
    /// Returns an error if the `subprotocol_id` or payload doesn’t match.
    fn from_section(section: &SectionState) -> Result<Box<dyn Subprotocol>, ASMError>
    where
        Self: Sized;

    /// Returns this subprotocol’s 1-byte (SPS-50) identifier.
    fn id(&self) -> u8;

    /// Process the L1Block and extracts all the relevant information from L1 for the subprotocol
    ///
    /// Update it's own state and output a list of InterProtoMsg addressed to other subprotocols
    fn process_block(&mut self, _block: &Block) -> Vec<(u8, InterProtoMsg)> {
        vec![]
    }

    /// Use the InterProtoMsg from other subprotocol to update it's state. Also generate the event
    /// logs that is later needed for introspection. Return the commitment of the events. The actual
    /// event is defined by the subprotocol and is not visible to the ASM.
    fn finalize_state(&mut self, _msgs: &[InterProtoMsg]) -> (SectionState, Buf32);
}
