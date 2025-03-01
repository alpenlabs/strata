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

fn gen_perf_report(host: &impl ZkVmHostPerf) -> PerformanceReport {
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
    use zkaleido::{VerifyingKey, ZkVmVerifier};
    use zkaleido_sp1_adapter::SP1Host;

    use super::*;

    fn host() -> impl ZkVmHostPerf {
        SP1Host::init(&GUEST_BTC_BLOCKSPACE_ELF)
    }

    pub fn perf_report() -> PerformanceReport {
        gen_perf_report(&host())
    }

    pub fn proof() -> ProofReceipt {
        gen_proof(&host())
    }

    pub fn vk() -> VerifyingKey {
        host().vk()
    }
}

#[cfg(feature = "risc0")]
pub(crate) mod risc0 {
    use strata_risc0_guest_builder::GUEST_RISC0_BTC_BLOCKSPACE_ELF;
    use zkaleido::{VerifyingKey, ZkVmVerifier};
    use zkaleido_risc0_adapter::Risc0Host;

    use super::*;

    fn host() -> impl ZkVmHostPerf {
        Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF)
    }

    pub fn perf_report() -> PerformanceReport {
        gen_perf_report(&host())
    }

    pub fn proof() -> ProofReceipt {
        gen_proof(&host())
    }

    pub fn vk() -> VerifyingKey {
        host().vk()
    }
}
