use strata_proofimpl_checkpoint::process_checkpoint_proof_outer;
use zkaleido_risc0_adapter::Risc0ZkVmEnv;

// TODO: replace this with vks file that'll generated by build.rs script similar to how things are
// implemented for sp1-guest-builder
pub const GUEST_CL_STF_ELF_ID: &[u32; 8] = &[0, 0, 0, 0, 0, 0, 0, 0];

fn main() {
    process_checkpoint_proof_outer(&Risc0ZkVmEnv, GUEST_CL_STF_ELF_ID);
}
