//! Subprotocol handler.

use std::{any::Any, collections::BTreeMap};

use strata_asm_common::{
    AsmError, InterprotoMsg, MsgRelayer, SectionState, Subprotocol, SubprotocolId, TxInput,
};

/// Subprotocol handler trait for a loaded subprotocol.
pub trait SubprotoHandler {
    /// Gets the ID of the subprotocol handler.
    fn id(&self) -> SubprotocolId;

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
    /// If an improper message type is provided.
    fn accept_msg(&mut self, msg: Box<dyn InterprotoMsg>);

    /// Processes the messages received.
    fn process_msgs(&mut self);

    /// Repacks the state into a [`SectionState`] instance.
    fn to_section(&self) -> SectionState;
}

/// Wrapper around the common subprotocol interface that handles the common
/// buffering logic for interproto messages.
pub struct HandlerImpl<S: Subprotocol, R> {
    state: S::State,
    interproto_msg_buf: Vec<S::Msg>,

    _r: std::marker::PhantomData<R>,
}

impl<S: Subprotocol + 'static, R: MsgRelayer + 'static> HandlerImpl<S, R> {
    pub fn new(state: S::State, interproto_msg_buf: Vec<S::Msg>) -> Self {
        Self {
            state,
            interproto_msg_buf,
            _r: std::marker::PhantomData,
        }
    }

    pub fn from_state(state: S::State) -> Self {
        Self::new(state, Vec::new())
    }

    /// Constructs an instance by trying to parse from a section's state.
    pub fn try_from_section(ss: &SectionState) -> Result<Self, AsmError> {
        ss.try_to_state::<S>().map(Self::from_state)
    }

    /// Converts the handler impl to a `Box<dyn Handler>` that we can use in the
    /// ASM executor.
    pub fn into_box_dyn(self) -> Box<dyn SubprotoHandler> {
        Box::new(self)
    }
}

impl<S: Subprotocol, R: MsgRelayer> SubprotoHandler for HandlerImpl<S, R> {
    fn id(&self) -> SubprotocolId {
        S::ID
    }

    fn accept_msg(&mut self, msg: Box<dyn InterprotoMsg>) {
        let m = msg
            .to_box_any()
            .downcast::<S::Msg>()
            .expect("asm: incorrect interproto msg type");
        self.interproto_msg_buf.push(*m);
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
pub struct HandlerRelayer {
    handlers: BTreeMap<SubprotocolId, Box<dyn SubprotoHandler>>,
}

impl HandlerRelayer {
    pub fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }

    /// Inserts a subproto by creating a handler for it.
    pub fn insert_subproto<S: Subprotocol>(&mut self, state: S::State) {
        let handler = HandlerImpl::<S, Self>::from_state(state);
        if self.handlers.insert(S::ID, Box::new(handler)).is_some() {
            panic!("asm: loaded state twice");
        }
    }

    fn get_handler(&mut self, id: SubprotocolId) -> Result<&Box<dyn SubprotoHandler>, AsmError> {
        self.handlers
            .get(&id)
            .ok_or(AsmError::InvalidSubprotocol(id))
    }

    fn get_handler_mut(
        &mut self,
        id: SubprotocolId,
    ) -> Result<&mut Box<dyn SubprotoHandler>, AsmError> {
        self.handlers
            .get_mut(&id)
            .ok_or(AsmError::InvalidSubprotocol(id))
    }
}

impl MsgRelayer for HandlerRelayer {
    fn relay_msg(&mut self, m: Box<dyn InterprotoMsg>) {
        let h = self
            .get_handler_mut(m.id())
            .expect("asm: msg to unloaded subprotocol");
        h.accept_msg(m);
    }

    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }
}
