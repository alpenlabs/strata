//! BridgeV1 Subprotocol
use borsh::{BorshDeserialize, BorshSerialize};
use strata_asm_common::{
    InterprotoMsg, MsgRelayer, NullMsg, SectionState, Subprotocol, SubprotocolId,
};
use strata_primitives::buf::Buf32;

/// The unique identifier for the BridgeV1 subprotocol within the Anchor State Machine.
///
/// This constant is used to tag `SectionState` entries belonging to the CoreASM logic
/// and must match the `subprotocol_id` checked in `SectionState::subprotocol()`.
pub const BRIDGE_V1_SUBPROTOCOL_ID: SubprotocolId = 2;

/// BridgeV1 state.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BridgeV1State {
    // TODO
}

/// BridgeV1 subprotocol impl.
#[derive(Copy, Clone, Debug)]
pub struct BridgeV1Subproto;

impl Subprotocol for BridgeV1Subproto {
    const ID: SubprotocolId = BRIDGE_V1_SUBPROTOCOL_ID;

    type State = BridgeV1State;

    type Msg = NullMsg<BRIDGE_V1_SUBPROTOCOL_ID>;

    fn init() -> Self::State {
        todo!()
    }

    fn process_txs(
        state: &mut Self::State,
        txs: &[strata_asm_common::TxInput<'_>],
        relayer: &mut impl strata_asm_common::MsgRelayer,
    ) {
        todo!()
    }

    fn finalize_state(state: &mut Self::State, msgs: &[Self::Msg]) {
        todo!()
    }
}
