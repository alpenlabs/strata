//! Subprotocol handler.

use std::{any::Any, collections::BTreeMap};

use strata_asm_common::{
    AsmError, InterprotoMsg, Log, MsgRelayer, SectionState, SubprotoHandler, Subprotocol,
    SubprotocolId, SubprotocolManager, TxInput,
};

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

impl SubprotocolManager for HandlerRelayer {
    /// Inserts a subproto by creating a handler for it.
    fn insert_subproto<S: Subprotocol>(&mut self, state: S::State) {
        let handler = HandlerImpl::<S, Self>::from_state(state);
        if self.handlers.insert(S::ID, Box::new(handler)).is_some() {
            panic!("asm: loaded state twice");
        }
    }

    fn insert_handler<S: Subprotocol>(&mut self, handler: Box<dyn SubprotoHandler>) {
        self.handlers.insert(S::ID, handler);
    }

    fn remove_handler(&mut self, id: SubprotocolId) -> Result<Box<dyn SubprotoHandler>, AsmError> {
        self.handlers
            .remove(&id)
            .ok_or(AsmError::InvalidSubprotocol(id))
    }

    fn get_handler(&self, id: SubprotocolId) -> Result<&dyn SubprotoHandler, AsmError> {
        self.handlers
            .get(&id)
            .map(Box::as_ref)
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

impl HandlerRelayer {
    pub(crate) fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
            logs: Vec::new(),
        }
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
