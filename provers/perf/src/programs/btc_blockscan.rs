use cfg_if::cfg_if;
use strata_l1tx::filter::TxFilterConfig;
use strata_proofimpl_btc_blockspace::{logic::BlockScanProofInput, program::BtcBlockspaceProgram};
use strata_test_utils::{bitcoin_mainnet_segment::BtcChainSegment, l2::gen_params};
use zkaleido::{PerformanceReport, ProofReceipt, ZkVmHostPerf, ZkVmProgram, ZkVmProgramPerf};

fn prepare_input() -> BlockScanProofInput {
    let params = gen_params();
    let rollup_params = params.rollup();
    let tx_filters = TxFilterConfig::derive_from(rollup_params).unwrap();

    let btc_blocks = vec![BtcChainSegment::load_full_block()];
    BlockScanProofInput {
        btc_blocks,
        tx_filters,
    }
}

fn btc_blockscan_perf_report(host: &impl ZkVmHostPerf) -> PerformanceReport {
    let input = prepare_input();
    BtcBlockspaceProgram::perf_report(&input, host).unwrap()
}

fn btc_blockscan_proof(host: &impl ZkVmHostPerf) -> ProofReceipt {
    let input = prepare_input();
    BtcBlockspaceProgram::prove(&input, host).unwrap()
}

#[cfg(feature = "sp1")]
pub fn sp1_btc_blockscan_report() -> PerformanceReport {
    use strata_sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;
    use zkaleido_sp1_adapter::SP1Host;
    let host = SP1Host::init(&GUEST_BTC_BLOCKSPACE_ELF);
    btc_blockscan_perf_report(&host)
}

#[cfg(feature = "sp1")]
pub fn sp1_btc_blockscan_proof() -> ProofReceipt {
    use strata_sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;
    use zkaleido_sp1_adapter::SP1Host;
    let host = SP1Host::init(&GUEST_BTC_BLOCKSPACE_ELF);
    btc_blockscan_proof(&host)
}

#[cfg(feature = "risc0")]
pub fn risc0_btc_blockscan_report() -> PerformanceReport {
    use strata_risc0_guest_builder::GUEST_RISC0_BTC_BLOCKSPACE_ELF;
    use zkaleido_risc0_adapter::Risc0Host;
    let host = Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF);
    btc_blockscan_perf_report(&host)
}

#[cfg(feature = "risc0")]
pub fn risc0_btc_blockscan_proof() -> ProofReceipt {
    use strata_risc0_guest_builder::GUEST_RISC0_BTC_BLOCKSPACE_ELF;
    use zkaleido_risc0_adapter::Risc0Host;
    let host = Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF);
    btc_blockscan_proof(&host)
}
