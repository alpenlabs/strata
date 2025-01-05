use strata_proofimpl_btc_blockspace::logic::process_blockspace_proof;
use strata_risc0_adapter::Risc0ZkVmEnv;

fn main() {
    process_blockspace_proof(&Risc0ZkVmEnv);
}
