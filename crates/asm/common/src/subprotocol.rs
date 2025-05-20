//! Subprotocol trait definition for ASM.
//!
//! This trait defines the interface every ASM subprotocol implementation must
//! provide. Each subprotocol is responsible for parsing its transactions,
//! updating its internal state, and emitting cross-protocol messages and logs.

use std::any::Any;

use bitcoin::Transaction;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::buf::Buf32;

use crate::{error::AsmError, msg::InterprotoMsg, state::SectionState};

/// Identifier for a subprotocol.
pub type SubprotocolId = u8;

/// ASM subprotocol interface.
///
/// A Subprotocol encapsulates a self-contained piece of logic that
///
/// 1. processes each new L1 block to update its own state and emit outgoing inter-protocol
///    messages, and then
/// 2. receives incoming messages to finalize and serialize its state for inclusion in the global
///    AnchorState.
pub trait Subprotocol: 'static {
    /// The subprotocol ID used when searching for relevant transactions.
    const ID: SubprotocolId;

    /// State type serialized into the ASM state structure.
    type State: Any + BorshDeserialize + BorshSerialize;

    /// Message type that we receive messages from other subprotocols using.
    type Msg: Any;

    /// Constructs a new state to use if the ASM does not have an instance of it.
    fn init() -> Self::State;

    /// Process the transactions and extract all the relevant information from L1 for the
    /// subprotocol
    ///
    /// Update it's own state and output a list of InterProtoMsg addressed to other subprotocols
    fn process_txs(state: &mut Self::State, txs: &[TxInput<'_>], relayer: &mut impl MsgRelayer);

    /// Use the msg other subprotocols to update its state. Also generate the event
    /// logs that is later needed for introspection. Return the commitment of the events. The actual
    /// event is defined by the subprotocol and is not visible to the ASM.
    fn finalize_state(state: &mut Self::State, msgs: &[Self::Msg]);
}

/// Generic message relayer interface.
pub trait MsgRelayer: Any {
    /// Relays a message to the destination subprotocol.
    fn relay_msg(&mut self, m: Box<dyn InterprotoMsg>);

    /// Gets this msg relayer as a `&dyn Any`.
    fn as_mut_any(&mut self) -> &mut dyn Any;
}

/// Transaction input data.
#[derive(Debug)]
pub struct TxInput<'t> {
    // TODO add tag data somehow so we don't have to re-extract it
    tx: &'t Transaction,
}

impl<'t> TxInput<'t> {
    /// Gets the inner transaction.
    pub fn tx(&self) -> &Transaction {
        self.tx
    }
}
