//! reusable utils.

use strata_primitives::params::Params;
use strata_state::{batch::SignedBatchCheckpoint, block_validation::verify_sequencer_signature};

/// Verify checkpoint has correct signature from sequencer.
pub fn verify_checkpoint_sig(signed_checkpoint: &SignedBatchCheckpoint, params: &Params) -> bool {
    let msg = signed_checkpoint.checkpoint().hash();
    let sig = signed_checkpoint.signature();
    verify_sequencer_signature(params.rollup(), &msg, &sig)
}
