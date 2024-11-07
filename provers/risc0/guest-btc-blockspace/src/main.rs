use strata_proofimpl_btc_blockspace::logic::process_blockspace_proof_outer;
use strata_risc0_adapter::ZkVmRisc0;

fn main() {
    process_blockspace_proof_outer(&ZkVmRisc0);
}
