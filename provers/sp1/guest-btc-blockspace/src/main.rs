use strata_proofimpl_btc_blockspace::logic::process_blockscan_proof;
use zkaleido_sp1_adapter::Sp1ZkVmEnv;

fn main() {
    process_blockscan_proof(&Sp1ZkVmEnv)
}
