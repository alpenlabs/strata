use strata_proofimpl_l1_batch::process_l1_batch_proof;
use strata_sp1_adapter::Sp1ZkVmEnv;

mod vks;

fn main() {
    process_l1_batch_proof(&Sp1ZkVmEnv, vks::GUEST_BTC_BLOCKSPACE_ELF_ID);
}
