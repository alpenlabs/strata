use strata_proofimpl_btc_blockspace::logic::process_blockspace_proof_outer;
use zkaleido_sp1_adapter::Sp1ZkVmEnv;

fn main() {
    process_blockspace_proof_outer(&Sp1ZkVmEnv)
}
