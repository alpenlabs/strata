//! Loader infrastructure for setting up the context.
// TODO maybe move (parts of) this module to common?

use std::collections::BTreeMap;

use strata_asm_common::{AnchorState, Subprotocol, SubprotocolId, TxInput};

use crate::handler::HandlerRelayer;

/// Specification for a concrete ASM instantation describing the subprotocols we
/// want to invoke and in what order.
///
/// This way, we only have to declare the subprotocols a single time and they
/// will always be processed in a consistent order as defined by an `AsmSpec`.
pub trait AsmSpec {
    /// Function that calls the loader with each subprotocol we intend to
    /// process, in the order we intend to process them.
    fn call_subprotocols(stage: &mut impl Stage);
}

/// Implementation of a subprotocol handling stage.
pub trait Stage {
    /// Invoked by the ASM spec to perform logic relating to a specific subprotocol.
    fn process_subprotocol<S: Subprotocol>(&mut self);
}

/// Stage that loads each subprotocol from the anchor state we're basing off of.
pub struct SubprotoLoaderStage<'a> {
    anchor_state: &'a AnchorState,
    handler: HandlerRelayer,
}

impl<'a> Stage for SubprotoLoaderStage<'a> {
    fn process_subprotocol<S: Subprotocol>(&mut self) {
        // Load or create the subprotocol state.
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
pub struct ProcessStage<'b> {
    tx_bufs: BTreeMap<SubprotocolId, Vec<TxInput<'b>>>,
    handler: HandlerRelayer,
}

impl<'b> Stage for ProcessStage<'b> {
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
pub struct FinishStage {
    handler: HandlerRelayer,
}

impl Stage for FinishStage {
    fn process_subprotocol<S: Subprotocol>(&mut self) {
        self.handler.invoke_process_msgs::<S>();
    }
}
