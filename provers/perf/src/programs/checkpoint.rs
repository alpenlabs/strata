use strata_proofimpl_checkpoint::program::{CheckpointProgram, CheckpointProverInput};
use strata_test_utils::evm_ee::EvmSegment;
use zkaleido::{PerformanceReport, ZkVmHostPerf, ZkVmProgramPerf};

fn prepare_input() -> CheckpointProverInput {
    let segment = EvmSegment::initialize_from_saved_ee_data(1, 3);
    segment.get_inputs().clone()
}

fn checkpoint_perf_report(host: &impl ZkVmHostPerf) -> PerformanceReport {
    let input = prepare_input();
    CheckpointProgram::perf_report(&input, host).unwrap()
}

#[cfg(feature = "sp1")]
pub fn sp1_checkpoint_report() -> PerformanceReport {
    use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use zkaleido_sp1_adapter::SP1Host;
    let host = SP1Host::init(&GUEST_EVM_EE_STF_ELF);
    checkpoint_perf_report(&host)
}

#[cfg(feature = "risc0")]
pub fn risc0_checkpoint_report() -> PerformanceReport {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use zkaleido_risc0_adapter::Risc0Host;
    let host = Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF);
    checkpoint_perf_report(&host)
}
