//! Subprotocol trait definition for ASM.
//!
//! This trait defines the interface every ASM subprotocol implementation must
//! provide. Each subprotocol is responsible for parsing its transactions,
//! updating its internal state, and emitting cross-protocol messages and logs.

use bitcoin::Transaction;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::buf::Buf32;

use crate::{SubprotocolId, error::ASMError, msg::InterProtoMsg, state::SectionState};

/// ASM subprotocol interface.
///
/// A Subprotocol encapsulates a self-contained piece of logic that
///
/// 1. processes each new L1 block to update its own state and emit outgoing inter-protocol
///    messages, and then
/// 2. receives incoming messages to finalize and serialize its state for inclusion in the global
///    AnchorState.
pub trait Subprotocol {
    /// Returns this subprotocol’s 1-byte (SPS-50) identifier.
    fn id(&self) -> SubprotocolId;

    /// Reconstructs your subprotocol instance from its prior `SectionState`.
    ///
    /// Returns an error if the `subprotocol_id` or payload doesn’t match.
    fn try_from_section(section: &SectionState) -> Result<Box<dyn Subprotocol>, ASMError>
    where
        Self: Sized + BorshDeserialize + 'static,
    {
        let inner: Self = BorshDeserialize::try_from_slice(&section.data)
            .map_err(|e| ASMError::Deserialization(section.id, e))?;
        Ok(Box::new(inner))
    }

    /// Serializes this subprotocol’s current state into a `SectionState` for inclusion
    /// in the global `AnchorState`.
    /// # Panics
    ///
    /// This will panic if Borsh serialization fails, which should never occur
    /// for a type that correctly derives `BorshSerialize`.
    fn to_section(&self) -> SectionState
    where
        Self: BorshSerialize + Sized,
    {
        let data = borsh::to_vec(&self).expect("Borsh serialization of Subprotocol state failed");
        SectionState {
            id: self.id(),
            data,
        }
    }

    /// Process the transactions and extract all the relevant information from L1 for the
    /// subprotocol
    ///
    /// Update it's own state and output a list of InterProtoMsg addressed to other subprotocols
    fn process_txs(&mut self, _txs: &[Transaction]) -> Vec<(u8, InterProtoMsg)> {
        vec![]
    }

    /// Use the InterProtoMsg from other subprotocol to update it's state. Also generate the event
    /// logs that is later needed for introspection. Return the commitment of the events. The actual
    /// event is defined by the subprotocol and is not visible to the ASM.
    fn finalize_state(&mut self, _msgs: &[InterProtoMsg]) -> (SectionState, Buf32);
}
