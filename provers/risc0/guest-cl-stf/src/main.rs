use strata_proofimpl_cl_stf::process_cl_stf;
use zkaleido_risc0_guest_env::Risc0ZkVmEnv;

// TODO: replace this with vks file that'll generated by build.rs script similar to how things are
// implemented for sp1-guest-builder
pub const GUEST_EVM_EE_STF_ELF_ID: &[u32; 8] = &[0, 0, 0, 0, 0, 0, 0, 0];

fn main() {
    process_cl_stf(&Risc0ZkVmEnv, GUEST_EVM_EE_STF_ELF_ID);
}
