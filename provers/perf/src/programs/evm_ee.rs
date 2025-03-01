use strata_proofimpl_evm_ee_stf::{primitives::EvmEeProofInput, program::EvmEeProgram};
use strata_test_utils::evm_ee::EvmSegment;
use zkaleido::{
    PerformanceReport, ProofReceipt, ZkVmHost, ZkVmHostPerf, ZkVmProgram, ZkVmProgramPerf,
};

fn prepare_input() -> EvmEeProofInput {
    let segment = EvmSegment::initialize_from_saved_ee_data(1, 3);
    segment.get_inputs().clone()
}

fn evm_ee_proof(host: &impl ZkVmHost) -> ProofReceipt {
    let input = prepare_input();
    EvmEeProgram::prove(&input, host).unwrap()
}

fn evm_ee_perf_report(host: &impl ZkVmHostPerf) -> PerformanceReport {
    let input = prepare_input();
    EvmEeProgram::perf_report(&input, host).unwrap()
}

#[cfg(feature = "sp1")]
pub fn sp1_evm_ee_report() -> PerformanceReport {
    use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use zkaleido_sp1_adapter::SP1Host;
    let host = SP1Host::init(&GUEST_EVM_EE_STF_ELF);
    evm_ee_perf_report(&host)
}

#[cfg(feature = "sp1")]
pub fn sp1_evm_ee_proof() -> ProofReceipt {
    use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use zkaleido_sp1_adapter::SP1Host;
    let host = SP1Host::init(&GUEST_EVM_EE_STF_ELF);
    evm_ee_proof(&host)
}

#[cfg(feature = "risc0")]
pub fn risc0_evm_ee_report() -> PerformanceReport {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use zkaleido_risc0_adapter::Risc0Host;
    let host = Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF);
    evm_ee_perf_report(&host)
}

#[cfg(feature = "risc0")]
pub fn risc0_evm_ee_proof() -> ProofReceipt {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use zkaleido_risc0_adapter::Risc0Host;
    let host = Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF);
    evm_ee_proof(&host)
}
