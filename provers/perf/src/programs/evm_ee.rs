use strata_proofimpl_evm_ee_stf::{primitives::EvmEeProofInput, program::EvmEeProgram};
use strata_test_utils::evm_ee::EvmSegment;
use tracing::info;
use zkaleido::{
    PerformanceReport, ProofReceipt, ZkVmHost, ZkVmHostPerf, ZkVmProgram, ZkVmProgramPerf,
};

fn prepare_input() -> EvmEeProofInput {
    info!("Preparing input for EVM EE STF");
    let segment = EvmSegment::initialize_from_saved_ee_data(1, 3);
    segment.get_inputs().clone()
}

fn gen_proof(host: &impl ZkVmHost) -> ProofReceipt {
    info!("Generating proof for EVM EE STF");
    let input = prepare_input();
    EvmEeProgram::prove(&input, host).unwrap()
}

fn gen_perf_report(host: &impl ZkVmHostPerf) -> PerformanceReport {
    info!("Generating performance report for EVM EE STF");
    let input = prepare_input();
    EvmEeProgram::perf_report(&input, host).unwrap()
}

#[cfg(feature = "sp1")]
pub mod sp1 {
    use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use zkaleido::{VerifyingKey, ZkVmVerifier};
    use zkaleido_sp1_adapter::SP1Host;

    use super::*;

    fn host() -> impl ZkVmHostPerf {
        SP1Host::init(&GUEST_EVM_EE_STF_ELF)
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
pub mod risc0 {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use zkaleido::{VerifyingKey, ZkVmVerifier};
    use zkaleido_risc0_adapter::Risc0Host;

    use super::*;

    fn host() -> impl ZkVmHostPerf {
        Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF)
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
