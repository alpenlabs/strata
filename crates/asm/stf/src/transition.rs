//! The `asm_stf` crate implements the core Anchor State Machine state transition function (STF). It
//! glues together block‐level validation, a set of pluggable subprotocols, and the global chain
//! view into a single deterministic state transition.

use std::collections::BTreeMap;

use bitcoin::{block::Block, params::Params};
use strata_asm_common::{AnchorState, ChainViewState};
use strata_asm_proto_bridge_v1::BridgeV1Subproto;
use strata_asm_proto_core::OLCoreSubproto;

use crate::stage::{AsmSpec, Stage};

/// ASM spec for the Strata protocol.
#[derive(Debug)]
pub struct StrataAsmSpec;

impl AsmSpec for StrataAsmSpec {
    fn call_subprotocols(stage: &mut impl Stage) {
        stage.process_subprotocol::<OLCoreSubproto>();
        stage.process_subprotocol::<BridgeV1Subproto>();
    }
}

/// Computes the next AnchorState by applying the Anchor State Machine (ASM) state transition
/// function (STF) to the given previous state and new L1 block.
pub fn asm_stf(pre_state: AnchorState, block: Block) -> AnchorState {
    // TODO a lot of this can reuse what's here but it needs to be adapted to
    // the above stage based design
    unimplemented!()
    /*
        // Extract mutable copies of the previous state's components
        let mut protocols = parse_subprotocols(&pre_state.sections);
        let mut pow_state = pre_state.chain_view.pow_state.clone();
        let mut events = pre_state.chain_view.events.clone();

        // 1. Validate and update PoW header continuity for the new block
        pow_state
            .check_and_update_continuity(&block.header, &Params::MAINNET)
            .expect("header doesn't follow the consensus rules");

        // 2. Filter the relevant transactions
        let all_relevant_transactions = group_txs_by_subprotocol(block.txdata);

        // 2. Process the relevant transaction of each subprotocol, gathering any emitted intermediate
        //    messages
        let mut inter_msgs = BTreeMap::new();
        for protocol in protocols.iter_mut() {
            let relevant_transactions = all_relevant_transactions
                .get(&protocol.id())
                .map(|v| v.as_slice())
                .unwrap_or_default();
            let msgs = protocol.process_txs(relevant_transactions);
            for (id, msg) in msgs {
                inter_msgs.entry(id).or_insert_with(Vec::new).push(msg);
            }
        }

        // 3. Finalize each subprotocol's state: obtain new section state and its MMR event hash
        let mut mmr_events = Vec::new();
        let mut sections = Vec::new();
        for protocol in protocols.iter_mut() {
            let id = protocol.id();
            let msgs = inter_msgs.entry(id).or_default();
            let (section, mmr_event_hash) = protocol.finalize_state(msgs);
            sections.push(section);
            mmr_events.push((id, mmr_event_hash));
        }

        // 4. Record this block's MMR events in the chain view, keyed by block ID
        events.push((*pow_state.last_verified_block.blkid(), mmr_events));

        // 5. Build the updated ChainViewState and return the new AnchorState
        let chain_view = ChainViewState { pow_state, events };
        AnchorState {
            chain_view,
            sections,
        }

    */
}
