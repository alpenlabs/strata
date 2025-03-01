use strata_proofimpl_cl_stf::program::{ClStfInput, ClStfProgram};
use strata_test_utils::{evm_ee::L2Segment, l2::gen_params};
use tracing::info;
use zkaleido::{
    PerformanceReport, ProofReceipt, VerifyingKey, ZkVmHost, ZkVmHostPerf, ZkVmProgram,
    ZkVmProgramPerf,
};

use super::evm_ee;

fn prepare_input(evm_ee_proof_with_vk: (ProofReceipt, VerifyingKey)) -> ClStfInput {
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

fn gen_perf_report(
    host: &impl ZkVmHostPerf,
    evm_ee_proof_with_vk: (ProofReceipt, VerifyingKey),
) -> PerformanceReport {
    info!("Generating performance report for CL STF");
    let input = prepare_input(evm_ee_proof_with_vk);
    ClStfProgram::perf_report(&input, host).unwrap()
}

fn gen_proof(
    host: &impl ZkVmHost,
    evm_ee_proof_with_vk: (ProofReceipt, VerifyingKey),
) -> ProofReceipt {
    info!("Generating proof for CL STF");
    let input = prepare_input(evm_ee_proof_with_vk);
    ClStfProgram::prove(&input, host).unwrap()
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

    fn evm_ee_proof_with_vk() -> (ProofReceipt, VerifyingKey) {
        let evm_ee_proof = evm_ee::sp1::proof();
        let vk = evm_ee::sp1::vk();
        (evm_ee_proof, vk)
    }

    pub fn perf_report() -> PerformanceReport {
        gen_perf_report(&host(), evm_ee_proof_with_vk())
    }

    pub fn proof() -> ProofReceipt {
        gen_proof(&host(), evm_ee_proof_with_vk())
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

    fn evm_ee_proof_with_vk() -> (ProofReceipt, VerifyingKey) {
        let evm_ee_proof = evm_ee::risc0::proof();
        let vk = evm_ee::risc0::vk();
        (evm_ee_proof, vk)
    }

    pub fn perf_report() -> PerformanceReport {
        gen_perf_report(&host(), evm_ee_proof_with_vk())
    }

    pub fn proof() -> ProofReceipt {
        gen_proof(&host(), evm_ee_proof_with_vk())
    }

    pub fn vk() -> VerifyingKey {
        host().vk()
    }
}
