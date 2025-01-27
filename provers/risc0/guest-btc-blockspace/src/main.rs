use strata_proofimpl_btc_blockspace::logic::process_blockspace_proof_outer;
use zkaleido_risc0_adapter::Risc0ZkVmEnv;

fn main() {
    process_blockspace_proof_outer(&Risc0ZkVmEnv);
}
