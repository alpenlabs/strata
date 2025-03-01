use strata_proofimpl_cl_stf::program::{ClStfInput, ClStfProgram};
use strata_test_utils::{
    evm_ee::{EvmSegment, L2Segment},
    l2::gen_params,
};
use zkaleido::{
    PerformanceReport, ProofReceipt, ZkVmHost, ZkVmHostPerf, ZkVmProgram, ZkVmProgramPerf,
};

fn prepare_input() -> ClStfInput {
    let params = gen_params();
    let rollup_params = params.rollup().clone();

    let evm_segment = EvmSegment::initialize_from_saved_ee_data(1, 3);
    let l2_segment = L2Segment::initialize_from_saved_evm_ee_data(1, 3);
    let chainstate = l2_segment.pre_states[0].clone();
    let l2_blocks = l2_segment.blocks.clone();

    ClStfInput {
        rollup_params,
        chainstate,
        l2_blocks,
    }
}

fn cl_stf_perf_report(host: &impl ZkVmHostPerf) -> PerformanceReport {
    let input = prepare_input();
    ClStfProgram::perf_report(&input, host).unwrap()
}

fn cl_stf_proof(host: &impl ZkVmHost) -> ProofReceipt {
    let input = prepare_input();
    ClStfProgram::prove(&input, host).unwrap()
}

#[cfg(feature = "sp1")]
pub fn sp1_cl_stf_report() -> PerformanceReport {
    use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use zkaleido_sp1_adapter::SP1Host;
    let host = SP1Host::init(&GUEST_EVM_EE_STF_ELF);
    cl_stf_perf_report(&host)
}

#[cfg(feature = "sp1")]
pub fn sp1_cl_stf_proof() -> ProofReceipt {
    use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use zkaleido_sp1_adapter::SP1Host;
    let host = SP1Host::init(&GUEST_EVM_EE_STF_ELF);
    cl_stf_proof(&host)
}

#[cfg(feature = "risc0")]
pub fn risc0_cl_stf_report() -> PerformanceReport {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use zkaleido_risc0_adapter::Risc0Host;
    let host = Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF);
    cl_stf_perf_report(&host)
}

#[cfg(feature = "risc0")]
pub fn risc0_cl_stf_proof() -> ProofReceipt {
    use strata_risc0_guest_builder::GUEST_RISC0_EVM_EE_STF_ELF;
    use zkaleido_risc0_adapter::Risc0Host;
    let host = Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF);
    cl_stf_proof(&host)
}
