use strata_proofimpl_cl_agg::process_cl_agg;
use strata_sp1_adapter::Sp1ZkVmEnv;

mod vks;

fn main() {
    process_cl_agg(&Sp1ZkVmEnv, vks::GUEST_CL_STF_ELF_ID)
}
