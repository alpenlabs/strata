//! Subprotocol trait definition for ASM.
//!
//! This trait defines the interface every ASM subprotocol implementation must
//! provide. Each subprotocol is responsible for parsing its transactions,
//! updating its internal state, and emitting cross-protocol messages and logs.

use bitcoin::Block;
use strata_primitives::buf::Buf32;

use crate::{msg::InterProtoMsg, state::SectionState};

/// Interface for ASM subprotocol implementations.
///
/// Each subprotocol must specify a unique `VERSION` and define the
/// associated `State`, `Msg`, `TxTag`, and `AuxInput` types. Processing
/// occurs in two phases: `process_block_txs` (initial pass) and
/// `finalize_state` (after inter-protocol messaging).
pub trait Subprotocol {
    /// 1-byte subprotocol identifier / version tag (matches SPS-50).
    const VERSION: u8;

    /// Returns the identifier of the subprotocol for this section.
    ///
    /// This ID corresponds to the version or namespace of the subprotocol whose
    /// state is serialized in this section.
    fn id(&self) -> u8 {
        Self::VERSION
    }

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
