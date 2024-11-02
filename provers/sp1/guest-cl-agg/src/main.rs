use strata_proofimpl_cl_agg::process_cl_agg;
use strata_sp1_adapter::ZkVmSp1;

mod vks;

fn main() {
    process_cl_agg(&ZkVmSp1, vks::GUEST_CL_STF_ELF_ID)
}
