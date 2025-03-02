use strata_proofimpl_evm_ee_stf::{primitives::EvmEeProofInput, program::EvmEeProgram};
use strata_test_utils::evm_ee::EvmSegment;
use tracing::info;
use zkaleido::{
    PerformanceReport, ProofReceipt, VerifyingKey, ZkVmHost, ZkVmHostPerf, ZkVmProgram,
    ZkVmProgramPerf,
};

pub fn prepare_input() -> EvmEeProofInput {
    info!("Preparing input for EVM EE STF");
    let segment = EvmSegment::initialize_from_saved_ee_data(1, 3);
    segment.get_inputs().clone()
}

fn gen_proof(host: &impl ZkVmHost) -> ProofReceipt {
    info!("Generating proof for EVM EE STF");
    let input = prepare_input();
    EvmEeProgram::prove(&input, host).unwrap()
}

pub fn gen_perf_report(host: &impl ZkVmHostPerf) -> PerformanceReport {
    info!("Generating performance report for EVM EE STF");
    let input = prepare_input();
    EvmEeProgram::perf_report(&input, host).unwrap()
}

pub fn proof_with_vk(host: &impl ZkVmHost) -> (ProofReceipt, VerifyingKey) {
    let proof = gen_proof(host);
    let vk = host.vk();
    (proof, vk)
}

#[cfg(feature = "sp1")]
pub mod sp1 {
    use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use zkaleido_sp1_adapter::SP1Host;

    use super::*;

    pub fn host() -> impl ZkVmHostPerf {
        SP1Host::init(&GUEST_EVM_EE_STF_ELF)
    }
}

#[cfg(feature = "risc0")]
pub mod risc0 {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use zkaleido_risc0_adapter::Risc0Host;

    use super::*;

    pub fn host() -> impl ZkVmHostPerf {
        Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evm_ee_native_execution() {
        let input = prepare_input();
        let output = EvmEeProgram::execute(&input).unwrap();
        dbg!(output);
    }
}
