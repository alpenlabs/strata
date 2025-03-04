use strata_proofimpl_cl_stf::program::{ClStfInput, ClStfProgram};
use strata_test_utils::{evm_ee::L2Segment, l2::gen_params};
use tracing::info;
use zkaleido::{
    PerformanceReport, ProofReceipt, VerifyingKey, ZkVmHost, ZkVmHostPerf, ZkVmProgram,
    ZkVmProgramPerf,
};

use super::evm_ee;

pub fn prepare_input(evm_ee_proof_with_vk: (ProofReceipt, VerifyingKey)) -> ClStfInput {
    info!("Preparing input for CL STF");
    let params = gen_params();
    let rollup_params = params.rollup().clone();

    let l2_segment = L2Segment::initialize_from_saved_evm_ee_data(1, 3);
    let chainstate = l2_segment.pre_states[0].clone();
    let l2_blocks = l2_segment.blocks.clone();

    ClStfInput {
        rollup_params,
        chainstate,
        l2_blocks,
        evm_ee_proof_with_vk,
        btc_blockspace_proof_with_vk: None,
    }
}

pub fn gen_perf_report(
    host: &impl ZkVmHostPerf,
    evm_ee_proof_with_vk: (ProofReceipt, VerifyingKey),
) -> PerformanceReport {
    info!("Generating performance report for CL STF");
    let input = prepare_input(evm_ee_proof_with_vk);
    ClStfProgram::perf_report(&input, host).unwrap()
}

pub fn gen_proof(
    host: &impl ZkVmHost,
    evm_ee_proof_with_vk: (ProofReceipt, VerifyingKey),
) -> ProofReceipt {
    info!("Generating proof for CL STF");
    let input = prepare_input(evm_ee_proof_with_vk);
    ClStfProgram::prove(&input, host).unwrap()
}

pub fn proof_with_vk(
    cl_stf_host: &impl ZkVmHost,
    evm_ee_host: &impl ZkVmHost,
) -> (ProofReceipt, VerifyingKey) {
    let evm_ee_proof_with_vk = evm_ee::proof_with_vk(evm_ee_host);

    let proof = gen_proof(cl_stf_host, evm_ee_proof_with_vk);
    (proof, cl_stf_host.vk())
}

#[cfg(feature = "sp1")]
pub mod sp1 {
    use strata_sp1_guest_builder::GUEST_CL_STF_ELF;
    use zkaleido_sp1_host::SP1Host;

    use super::*;

    pub fn host() -> impl ZkVmHostPerf {
        SP1Host::init(&GUEST_CL_STF_ELF)
    }
}

#[cfg(feature = "risc0")]
pub mod risc0 {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use zkaleido_risc0_host::Risc0Host;

    use super::*;

    pub fn host() -> impl ZkVmHostPerf {
        Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF)
    }
}

#[cfg(test)]
mod tests {
    use strata_proofimpl_evm_ee_stf::program::EvmEeProgram;

    use super::*;

    #[test]
    fn test_cl_stf_native_execution() {
        let evm_ee_proof_with_vk = evm_ee::proof_with_vk(&EvmEeProgram::native_host());
        let input = prepare_input(evm_ee_proof_with_vk);
        let output = ClStfProgram::execute(&input).unwrap();
        dbg!(output);
    }
}
