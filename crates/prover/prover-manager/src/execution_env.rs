use alpen_vertex_primitives::prelude::Buf32;
use alpen_vertex_state::exec_update::{ExecUpdate, UpdateInput, UpdateOutput};
use zkvm_primitives::ELProofPublicParams;

pub fn el_proof_to_exec_update(params: &ELProofPublicParams) -> ExecUpdate {
    let update_input = UpdateInput::new(params.block_idx, Buf32(params.txn_root), Vec::new());
    let update_output = UpdateOutput::new_from_state(Buf32(params.new_state_root));

    ExecUpdate::new(update_input, update_output)
}
