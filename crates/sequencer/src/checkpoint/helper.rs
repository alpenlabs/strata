use strata_crypto::verify_schnorr_sig;
use strata_primitives::{block_credential::CredRule, params::Params};
use strata_state::batch::SignedBatchCheckpoint;

pub fn verify_checkpoint_sig(signed_checkpoint: &SignedBatchCheckpoint, params: &Params) -> bool {
    match params.rollup().cred_rule {
        CredRule::Unchecked => true,
        CredRule::SchnorrKey(pubkey) => verify_schnorr_sig(
            &signed_checkpoint.signature(),
            &signed_checkpoint.checkpoint().hash(),
            &pubkey,
        ),
    }
}
