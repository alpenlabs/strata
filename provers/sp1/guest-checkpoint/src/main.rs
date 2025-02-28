use strata_proofimpl_checkpoint::process_checkpoint_proof_outer;
use zkaleido_sp1_adapter::Sp1ZkVmEnv;

mod vks;

fn main() {
    process_checkpoint_proof_outer(&Sp1ZkVmEnv, vks::GUEST_CL_STF_ELF_ID)
}
