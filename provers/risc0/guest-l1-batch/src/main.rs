use strata_proofimpl_l1_batch::process_l1_batch_proof;
use zkaleido_risc0_adapter::Risc0ZkVmEnv;

fn main() {
    process_l1_batch_proof(&Risc0ZkVmEnv);
}
