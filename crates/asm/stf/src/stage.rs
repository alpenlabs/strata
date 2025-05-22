//! Loader infrastructure for setting up the context.
// TODO maybe move (parts of) this module to common?

use std::collections::BTreeMap;

use strata_asm_common::{
    AnchorState, SectionState, Stage, Subprotocol, SubprotocolId, SubprotocolManager, TxInput,
};

/// Stage that loads each subprotocol from the anchor state we're basing off of.
pub(crate) struct SubprotoLoaderStage<'a> {
    anchor_state: &'a AnchorState,
}

impl<'a> SubprotoLoaderStage<'a> {
    pub(crate) fn new(anchor_state: &'a AnchorState) -> Self {
        Self { anchor_state }
    }
}

impl Stage for SubprotoLoaderStage<'_> {
    fn process_subprotocol<S: Subprotocol>(&mut self, manager: &mut impl SubprotocolManager) {
        // Load or create the subprotocol state.
        // OPTIMIZE: Linear scan is done every time to find the section
        let state = match self.anchor_state.find_section(S::ID) {
            Some(sec) => sec
                .try_to_state::<S>()
                .expect("asm: invalid section subproto state"),
            None => S::init(),
        };

        manager.insert_subproto::<S>(state);
    }
}

/// Stage to process txs pre-extracted from the block for each subprotocol.
pub(crate) struct ProcessStage<'b> {
    tx_bufs: BTreeMap<SubprotocolId, Vec<TxInput<'b>>>,
}

impl<'b> ProcessStage<'b> {
    pub(crate) fn new(tx_bufs: BTreeMap<SubprotocolId, Vec<TxInput<'b>>>) -> Self {
        Self { tx_bufs }
    }
}

impl Stage for ProcessStage<'_> {
    fn process_subprotocol<S: Subprotocol>(&mut self, manager: &mut impl SubprotocolManager) {
        let txs = self
            .tx_bufs
            .get(&S::ID)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        manager.invoke_process_txs::<S>(txs);
    }
}

/// Stage to handle messages exchanged between subprotocols in execution.
pub(crate) struct FinishStage {
    sections: Vec<SectionState>,
}

impl FinishStage {
    pub(crate) fn new() -> Self {
        let sections = Vec::new();
        Self { sections }
    }

    pub(crate) fn into_sections(self) -> Vec<SectionState> {
        self.sections
    }
}

impl Stage for FinishStage {
    fn process_subprotocol<S: Subprotocol>(&mut self, manager: &mut impl SubprotocolManager) {
        manager.invoke_process_msgs::<S>();
        let section = manager.to_section_state::<S>();
        self.sections.push(section);
    }
}
