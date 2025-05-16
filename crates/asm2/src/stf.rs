use std::collections::HashMap;

use bitcoin::{block::Block, params::Params};

use crate::{
    state::{AnchorState, ChainViewState},
    subprotocol::Subprotocol,
};

/// Computes the next AnchorState by applying the Anchor State Machine (ASM) state transition
/// function (STF) to the given previous state and new L1 block.
pub fn asm_stf(pre_state: AnchorState, block: Block) -> AnchorState {
    // Extract mutable copies of the previous state's components
    let mut protocols = pre_state.subprotocols();
    let mut pow_state = pre_state.chain_view.pow_state.clone();
    let mut events = pre_state.chain_view.events.clone();

    // 1. Validate and update PoW header continuity for the new block
    pow_state
        .check_and_update_continuity(&block.header, &Params::MAINNET)
        .expect("header doesn't follow the consensus rules");

    // 2. Process the block in each subprotocol, gathering any emitted intermediate messages
    let mut inter_msgs = HashMap::new();
    for protocol in protocols.iter_mut() {
        let msgs = protocol.process_block(&block);
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
}
