// I worry that this design is broken down a bit too much and would lead to more
// RPC traffic with the EL node if we do go down and just make this be a wrapper
// to the engine API.  Perhaps we should make this a message oriented
// architecture where we produce a bundle of changes to update our expected idea
// of the EL state and then after applying a bundle of changes we can then go
// and update the external EL state.  That would mean going and making a more
// minimal set of EL calls.  This design would probably be more able to handle
// errors as we'd be able to identify when our perspective on the state is
// inconsistent with the remote state.

use alpen_vertex_primitives::buf::Buf32;

use crate::errors::*;
use crate::messages::*;

/// Interface to control an execution engine.  This is defined in terms of
/// Vertex semantics which will be produced inside the EL impl according to
/// whatever semantics it has.
pub trait ExecEngineCtl {
    /// Updates the EL payload chain tip that we should be trying to execute to
    /// determine validity.
    fn update_head_block(&self, id: Buf32) -> EngineResult<BlockStatus>;

    /// Updates the block that we've considered full buried.  This means it's
    /// been proven on-chain sufficiently that it will never be rolled back.
    fn update_finalized_block(&self, id: Buf32) -> EngineResult<()>;

    /// Tries to prepare a payload using the current state of the chain,
    /// returning an ID to query pending payload build jobs.  If this completes
    /// successfully and then `.update_head_block` is called on it, will
    /// broadcast new payload to peers.
    fn prepare_payload(&self, env: PayloadEnv) -> EngineResult<u64>;

    /// Tries to get a payload that we were working on.
    fn get_payload_status(&self, id: u64) -> EngineResult<PayloadStatus>;

    // TODO more stuff to ensure that the EL actually gets the payloads and the
    // CL context it needs to execute the blocks
}

/// The status of a block that we've just set fork choice fork.
///
/// Corresponds to `Forkchoice
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BlockStatus {
    /// The block tip is valid.
    Valid,

    /// The block tip is invalid, reason doesn't matter.
    Invalid,

    /// We are still syncing previous blocks and don't have the ability to
    /// figure out this query yet.
    Syncing,
}

pub enum PayloadStatus {
    /// Still building the payload.
    Working,

    /// Completed, with short commitment payload data.
    Ready(ExecPayloadData),
}
