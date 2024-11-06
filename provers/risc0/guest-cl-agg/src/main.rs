use strata_proofimpl_cl_agg::process_cl_agg;
use strata_risc0_adapter::Risc0ZkVmEnv;

// TODO: replace this with vks file that'll generated by build.rs script similar to how things are
// implemented for sp1-guest-builder
pub const GUEST_CL_STF_ELF_ID: &[u32; 8] = &[0, 0, 0, 0, 0, 0, 0, 0];

fn main() {
    process_cl_agg(&Risc0ZkVmEnv, GUEST_CL_STF_ELF_ID)
}
