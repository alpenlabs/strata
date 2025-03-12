use strata_proofimpl_evm_ee_stf::process_block_transaction_outer;
use zkaleido_risc0_guest_env::Risc0ZkVmEnv;

fn main() {
    process_block_transaction_outer(&Risc0ZkVmEnv);
}
