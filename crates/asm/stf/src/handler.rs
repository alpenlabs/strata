//! Subprotocol handler.

use std::{any::Any, collections::BTreeMap};

use strata_asm_common::{
    AsmError, InterprotoMsg, Log, MsgRelayer, SectionState, Subprotocol, SubprotocolId, TxInput,
};

/// Subprotocol handler trait for a loaded subprotocol.
pub(crate) trait SubprotoHandler {
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

/// Wrapper around the common subprotocol interface that handles the common
/// buffering logic for interproto messages.
pub(crate) struct HandlerImpl<S: Subprotocol, R> {
    state: S::State,
    interproto_msg_buf: Vec<S::Msg>,

    _r: std::marker::PhantomData<R>,
}

impl<S: Subprotocol + 'static, R: MsgRelayer + 'static> HandlerImpl<S, R> {
    pub(crate) fn new(state: S::State, interproto_msg_buf: Vec<S::Msg>) -> Self {
        Self {
            state,
            interproto_msg_buf,
            _r: std::marker::PhantomData,
        }
    }

    pub(crate) fn from_state(state: S::State) -> Self {
        Self::new(state, Vec::new())
    }
}

impl<S: Subprotocol, R: MsgRelayer> SubprotoHandler for HandlerImpl<S, R> {
    fn accept_msg(&mut self, msg: &dyn InterprotoMsg) {
        let m = msg
            .as_dyn_any()
            .downcast_ref::<S::Msg>()
            .expect("asm: incorrect interproto msg type");
        self.interproto_msg_buf.push(m.clone());
    }

    fn process_txs(&mut self, txs: &[TxInput<'_>], relayer: &mut dyn MsgRelayer) {
        let relayer = relayer
            .as_mut_any()
            .downcast_mut::<R>()
            .expect("asm: handler");
        S::process_txs(&mut self.state, txs, relayer);
    }

    fn process_msgs(&mut self) {
        S::finalize_state(&mut self.state, &self.interproto_msg_buf)
    }

    fn to_section(&self) -> SectionState {
        SectionState::from_state::<S>(&self.state)
    }
}

/// Executor that manages a set of loaded subprotocols.
pub(crate) struct HandlerRelayer {
    handlers: BTreeMap<SubprotocolId, Box<dyn SubprotoHandler>>,
    logs: Vec<Log>,
}

impl HandlerRelayer {
    pub(crate) fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
            logs: Vec::new(),
        }
    }

    /// Inserts a subproto by creating a handler for it.
    pub(crate) fn insert_subproto<S: Subprotocol>(&mut self, state: S::State) {
        let handler = HandlerImpl::<S, Self>::from_state(state);
        if self.handlers.insert(S::ID, Box::new(handler)).is_some() {
            panic!("asm: loaded state twice");
        }
    }

    pub(crate) fn get_handler(&self, id: SubprotocolId) -> Result<&dyn SubprotoHandler, AsmError> {
        self.handlers
            .get(&id)
            .map(Box::as_ref)
            .ok_or(AsmError::InvalidSubprotocol(id))
    }

    pub(crate) fn get_handler_mut(
        &mut self,
        id: SubprotocolId,
    ) -> Result<&mut Box<dyn SubprotoHandler>, AsmError> {
        self.handlers
            .get_mut(&id)
            .ok_or(AsmError::InvalidSubprotocol(id))
    }

    pub(crate) fn invoke_process_txs<S: Subprotocol>(&mut self, txs: &[TxInput<'_>]) {
        // We temporarily take the handler out of the map so we can call
        // `process_txs` with `self` as the relayer without violating the
        // borrow checker.
        let mut h = self
            .handlers
            .remove(&S::ID)
            .expect("asm: unloaded subprotocol");
        h.process_txs(txs, self);
        self.handlers.insert(S::ID, h);
    }

    pub(crate) fn invoke_process_msgs<S: Subprotocol>(&mut self) {
        let h = self
            .get_handler_mut(S::ID)
            .expect("asm: unloaded subprotocol");
        h.process_msgs()
    }

    pub(crate) fn to_section_state<S: Subprotocol>(&self) -> SectionState {
        let h = self.get_handler(S::ID).expect("asm: unloaded subprotocol");
        h.to_section()
    }
}

impl MsgRelayer for HandlerRelayer {
    fn relay_msg(&mut self, m: &dyn InterprotoMsg) {
        let h = self
            .get_handler_mut(m.id())
            .expect("asm: msg to unloaded subprotocol");
        h.accept_msg(m);
    }

    fn emit_log(&mut self, log: Log) {
        self.logs.push(log);
    }

    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }
}
