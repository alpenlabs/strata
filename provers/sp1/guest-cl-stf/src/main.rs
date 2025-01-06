use strata_proofimpl_cl_stf::batch_process_cl_stf;
use strata_sp1_adapter::Sp1ZkVmEnv;

mod vks;

fn main() {
    batch_process_cl_stf(&Sp1ZkVmEnv, vks::GUEST_EVM_EE_STF_ELF_ID);
}
