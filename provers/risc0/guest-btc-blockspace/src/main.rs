use strata_proofimpl_btc_blockspace::logic::process_blockscan_proof;
use zkaleido_risc0_adapter::Risc0ZkVmEnv;

fn main() {
    process_blockscan_proof(&Risc0ZkVmEnv);
}
