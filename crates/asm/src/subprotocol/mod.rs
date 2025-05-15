//! Subprotocol trait definition for ASM.
//!
//! This trait defines the interface every ASM subprotocol implementation must
//! provide. Each subprotocol is responsible for parsing its transactions,
//! updating its internal state, and emitting cross-protocol messages and logs.

use bitcoin::Block;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::l1::ProtocolOperation;

use crate::{
    SectionState,
    msg::{InterProtoMsg, ProtoEvent},
};

/// Interface for ASM subprotocol implementations.
///
/// Each subprotocol must specify a unique `VERSION` and define the
/// associated `State`, `Msg`, `TxTag`, and `AuxInput` types. Processing
/// occurs in two phases: `process_block_txs` (initial pass) and
/// `finalize_state` (after inter-protocol messaging).
pub trait Subprotocol {
    /// 1-byte subprotocol identifier / version tag (matches SPS-50).
    const VERSION: u8;

    /// Process the L1Block and extracts all the relevant information from L1 for the subprotocol
    ///
    /// Update it's own output and as output this should give a list of InterProtoMsg addressed to
    /// other subprotocol
    fn process_block(&mut self, _block: &Block) -> Vec<(u8, InterProtoMsg)> {
        vec![]
    }

    /// Use the InterProtoMsg from other subprotocol to update it's state and generate the
    /// ProtoEvent
    fn finalize_state(&mut self, _msgs: &[InterProtoMsg]) -> Vec<ProtoEvent> {
        vec![]
    }

    fn to_section_state(&self) -> SectionState;
}

pub mod bridge;
pub mod core;
