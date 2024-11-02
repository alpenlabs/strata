use strata_proofimpl_checkpoint::process_checkpoint_proof_outer;
use strata_sp1_adapter::ZkVmSp1;

mod vks;

fn main() {
    process_checkpoint_proof_outer(
        &ZkVmSp1,
        vks::GUEST_L1_BATCH_ELF_ID,
        vks::GUEST_CL_AGG_ELF_ID,
    )
}
