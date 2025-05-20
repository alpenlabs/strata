//! Loader infrastructure for setting up the context.
// TODO maybe move (parts of) this module to common?

use std::collections::BTreeMap;

use strata_asm_common::{AnchorState, SectionState, Stage, Subprotocol, SubprotocolId, TxInput};

use crate::handler::HandlerRelayer;

/// Stage that loads each subprotocol from the anchor state we're basing off of.
pub(crate) struct SubprotoLoaderStage<'a> {
    anchor_state: &'a AnchorState,
    handler: HandlerRelayer,
}

impl<'a> SubprotoLoaderStage<'a> {
    pub(crate) fn new(anchor_state: &'a AnchorState) -> Self {
        Self {
            anchor_state,
            handler: HandlerRelayer::new(),
        }
    }

    pub(crate) fn into_handler(self) -> HandlerRelayer {
        self.handler
    }
}

impl Stage for SubprotoLoaderStage<'_> {
    fn process_subprotocol<S: Subprotocol>(&mut self) {
        // Load or create the subprotocol state.
        // OPTIMIZE: Linear scan is done every time to find the section
        let state = match self.anchor_state.find_section(S::ID) {
            Some(sec) => sec
                .try_to_state::<S>()
                .expect("asm: invalid section subproto state"),
            None => S::init(),
        };

        self.handler.insert_subproto::<S>(state);
    }
}

/// Stage to process txs pre-extracted from the block for each subprotocol.
pub(crate) struct ProcessStage<'b> {
    tx_bufs: BTreeMap<SubprotocolId, Vec<TxInput<'b>>>,
    handler: HandlerRelayer,
}

impl<'b> ProcessStage<'b> {
    pub(crate) fn new(
        tx_bufs: BTreeMap<SubprotocolId, Vec<TxInput<'b>>>,
        handler: HandlerRelayer,
    ) -> Self {
        Self { tx_bufs, handler }
    }

    pub(crate) fn into_handler(self) -> HandlerRelayer {
        self.handler
    }
}

impl Stage for ProcessStage<'_> {
    fn process_subprotocol<S: Subprotocol>(&mut self) {
        let txs = self
            .tx_bufs
            .get(&S::ID)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        self.handler.invoke_process_txs::<S>(txs);
    }
}

/// Stage to handle messages exchanged between subprotocols in execution.
pub(crate) struct FinishStage {
    handler: HandlerRelayer,
    sections: Vec<SectionState>,
}

impl FinishStage {
    pub(crate) fn new(handler: HandlerRelayer) -> Self {
        let sections = Vec::new();
        Self { handler, sections }
    }

    pub(crate) fn into_sections(self) -> Vec<SectionState> {
        self.sections
    }
}

impl Stage for FinishStage {
    fn process_subprotocol<S: Subprotocol>(&mut self) {
        self.handler.invoke_process_msgs::<S>();
        let section = self.handler.to_section_state::<S>();
        self.sections.push(section);
    }
}
