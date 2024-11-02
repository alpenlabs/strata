use strata_proofimpl_l1_batch::process_l1_batch_proof;
use strata_sp1_adapter::ZkVmSp1;

mod vks;

fn main() {
    process_l1_batch_proof(&ZkVmSp1, vks::GUEST_BTC_BLOCKSPACE_ELF_ID);
}
