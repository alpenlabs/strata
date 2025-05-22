//! Subprotocol trait definition for ASM.
//!
//! This trait defines the interface every ASM subprotocol implementation must
//! provide. Each subprotocol is responsible for parsing its transactions,
//! updating its internal state, and emitting cross-protocol messages and logs.

use std::any::Any;

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{AsmError, Log, SectionState, TxInput, msg::InterprotoMsg};

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
    type Msg: Clone + Any;

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
    fn relay_msg(&mut self, m: &dyn InterprotoMsg);

    /// Emits an output log message.
    fn emit_log(&mut self, log: Log);

    /// Gets this msg relayer as a `&dyn Any`.
    fn as_mut_any(&mut self) -> &mut dyn Any;
}

/// Subprotocol handler trait for a loaded subprotocol.
pub trait SubprotoHandler {
    /// Processes transactions that were previously collected.
    fn process_txs(&mut self, txs: &[TxInput<'_>], relayer: &mut dyn MsgRelayer);

    /// Accepts a message.  This is called while processing other subprotocols.
    /// These should not be processed until we do the finalization.
    ///
    /// This MUST NOT act on any messages that were accepted before this was
    /// called.
    ///
    /// # Panics
    ///
    /// If an mismatched message type (behind the `dyn`) is provided.
    fn accept_msg(&mut self, msg: &dyn InterprotoMsg);

    /// Processes the messages received.
    fn process_msgs(&mut self);

    /// Repacks the state into a [`SectionState`] instance.
    fn to_section(&self) -> SectionState;
}

/// Manages the lifecycle and execution of subprotocol handlers in the Anchor State Machine (ASM).
///
/// Implementors of this trait maintain a collection of subprotocol handlers and
/// provide methods to insert, remove, lookup, and drive execution (transactions and messages),
/// as well as extract the final `SectionState`.
pub trait SubprotocolManager: MsgRelayer + Sized {
    /// Inserts a new subprotocol by consuming its initial state and creating its handler.
    fn insert_subproto<S: Subprotocol>(&mut self, state: S::State);

    /// Inserts a boxed handler directly.
    fn insert_handler<S: Subprotocol>(&mut self, handler: Box<dyn SubprotoHandler>);

    /// Removes and returns the handler for the given `id`.
    ///
    /// # Errors
    ///
    /// Returns `AsmError::InvalidSubprotocol(id)` if no handler with that ID is present.
    fn remove_handler(&mut self, id: SubprotocolId) -> Result<Box<dyn SubprotoHandler>, AsmError>;

    /// Retrieves an immutable reference to the handler for the given `id`.
    ///
    /// # Errors
    ///
    /// Returns `AsmError::InvalidSubprotocol(id)` if no handler with that ID is present.
    fn get_handler(&self, id: SubprotocolId) -> Result<&dyn SubprotoHandler, AsmError>;

    /// Retrieves a mutable reference to the handler for the given `id`.
    ///
    /// # Errors
    ///
    /// Returns `AsmError::InvalidSubprotocol(id)` if no handler with that ID is present.
    fn get_handler_mut(
        &mut self,
        id: SubprotocolId,
    ) -> Result<&mut Box<dyn SubprotoHandler>, AsmError>;

    /// Dispatches transaction processing to the appropriate handler.
    ///
    /// This default implementation temporarily removes the handler to satisfy
    /// borrow-checker constraints, invokes `process_txs` with `self` as the relayer,
    /// and then reinserts the handler.
    fn invoke_process_txs<S: Subprotocol>(&mut self, txs: &[TxInput<'_>]) {
        // We temporarily take the handler out of the map so we can call
        // `process_txs` with `self` as the relayer without violating the
        // borrow checker.
        let mut h = self
            .remove_handler(S::ID)
            .expect("asm: unloaded subprotocol");
        h.process_txs(txs, self);
        self.insert_handler::<S>(h);
    }

    /// Dispatches buffered inter-protocol message processing to the handler.
    fn invoke_process_msgs<S: Subprotocol>(&mut self) {
        let h = self
            .get_handler_mut(S::ID)
            .expect("asm: unloaded subprotocol");
        h.process_msgs()
    }

    /// Extracts the finalized `SectionState` from the handler.
    fn to_section_state<S: Subprotocol>(&self) -> SectionState {
        let h = self.get_handler(S::ID).expect("asm: unloaded subprotocol");
        h.to_section()
    }
}
