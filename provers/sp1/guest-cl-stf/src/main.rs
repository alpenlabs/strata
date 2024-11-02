use strata_proofimpl_cl_stf::process_cl_stf;
use strata_sp1_adapter::ZkVmSp1;

mod vks;

fn main() {
    process_cl_stf(&ZkVmSp1, vks::GUEST_EVM_EE_STF_ELF_ID);
}
