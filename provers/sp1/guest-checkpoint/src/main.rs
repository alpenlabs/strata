use strata_proofimpl_checkpoint::process_checkpoint_proof_outer;
use zkaleido_sp1_adapter::Sp1ZkVmEnv;

mod vks;

fn main() {
    process_checkpoint_proof_outer(
        &Sp1ZkVmEnv,
        vks::GUEST_L1_BATCH_ELF_ID,
        vks::GUEST_CL_AGG_ELF_ID,
    )
}
