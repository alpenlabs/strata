//! The `asm_stf` crate implements the core Anchor State Machine state transition function (STF). It
//! glues together block‐level validation, a set of pluggable subprotocols, and the global chain
//! view into a single deterministic state transition.

use bitcoin::{block::Block, params::Params};
use strata_asm_common::{AnchorState, AsmSpec, ChainViewState, Stage, SubprotocolManager};
use strata_asm_proto_bridge_v1::BridgeV1Subproto;
use strata_asm_proto_core::OLCoreSubproto;

use crate::{
    handler::HandlerRelayer,
    stage::{FinishStage, ProcessStage, SubprotoLoaderStage},
    tx_filter::group_txs_by_subprotocol,
};

/// ASM spec for the Strata protocol.
#[derive(Debug)]
pub struct StrataAsmSpec;

impl AsmSpec for StrataAsmSpec {
    fn call_subprotocols(stage: &mut impl Stage, manager: &mut impl SubprotocolManager) {
        stage.process_subprotocol::<OLCoreSubproto>(manager);
        stage.process_subprotocol::<BridgeV1Subproto>(manager);
    }
}

/// Computes the next AnchorState by applying the Anchor State Machine (ASM) state transition
/// function (STF) to the given previous state and new L1 block.
pub fn asm_stf<S: AsmSpec>(pre_state: AnchorState, block: Block) -> AnchorState {
    let mut pow_state = pre_state.chain_view.pow_state.clone();

    // 1. Validate and update PoW header continuity for the new block
    pow_state
        .check_and_update_continuity(&block.header, &Params::MAINNET)
        .expect("header doesn't follow the consensus rules");

    // 2. Filter the relevant transactions
    let all_relevant_transactions = group_txs_by_subprotocol(&block.txdata);

    let mut manager = HandlerRelayer::new();

    // 3. LOAD: bring each subprotocol into a HandlerRelayer
    let mut loader_stage = SubprotoLoaderStage::new(&pre_state);
    S::call_subprotocols(&mut loader_stage, &mut manager);

    // 4. PROCESS: feed each subprotocol its slice of txs
    let mut process_stage = ProcessStage::new(all_relevant_transactions);
    S::call_subprotocols(&mut process_stage, &mut manager);

    // 5. FINISH: let each subprotocol process its buffered interproto messages
    let mut finish_stage = FinishStage::new();
    S::call_subprotocols(&mut finish_stage, &mut manager);

    let sections = finish_stage.into_sections();
    let chain_view = ChainViewState { pow_state };
    AnchorState {
        chain_view,
        sections,
    }
}
