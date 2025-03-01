use strata_l1tx::filter::TxFilterConfig;
use strata_proofimpl_btc_blockspace::{logic::BlockScanProofInput, program::BtcBlockspaceProgram};
use strata_test_utils::{bitcoin_mainnet_segment::BtcChainSegment, l2::gen_params};
use tracing::info;
use zkaleido::{PerformanceReport, ProofReceipt, ZkVmHostPerf, ZkVmProgram, ZkVmProgramPerf};

fn prepare_input() -> BlockScanProofInput {
    info!("Preparing input for BTC Blockcan");
    let params = gen_params();
    let rollup_params = params.rollup();
    let tx_filters = TxFilterConfig::derive_from(rollup_params).unwrap();

    let btc_blocks = vec![BtcChainSegment::load_full_block()];
    BlockScanProofInput {
        btc_blocks,
        tx_filters,
    }
}

pub fn gen_perf_report(host: &impl ZkVmHostPerf) -> PerformanceReport {
    info!("Generating performance report for BTC Blockcan");
    let input = prepare_input();
    BtcBlockspaceProgram::perf_report(&input, host).unwrap()
}

fn gen_proof(host: &impl ZkVmHostPerf) -> ProofReceipt {
    info!("Generating proof for BTC Blockcan");
    let input = prepare_input();
    BtcBlockspaceProgram::prove(&input, host).unwrap()
}

#[cfg(feature = "sp1")]
pub(crate) mod sp1 {
    use strata_sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;
    use zkaleido_sp1_adapter::SP1Host;

    use super::*;

    pub fn host() -> impl ZkVmHostPerf {
        SP1Host::init(&GUEST_BTC_BLOCKSPACE_ELF)
    }
}

#[cfg(feature = "risc0")]
pub(crate) mod risc0 {
    use strata_risc0_guest_builder::GUEST_RISC0_BTC_BLOCKSPACE_ELF;
    use zkaleido_risc0_adapter::Risc0Host;

    use super::*;

    pub fn host() -> impl ZkVmHostPerf {
        Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btc_blockscan_execution() {
        let input = prepare_input();
        let output = BtcBlockspaceProgram::execute(&input).unwrap();
        dbg!(output);
    }
}
