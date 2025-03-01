use strata_proofimpl_checkpoint::program::{CheckpointProgram, CheckpointProverInput};
use zkaleido::{
    PerformanceReport, ProofReceipt, VerifyingKey, ZkVmHost, ZkVmHostPerf, ZkVmProgram,
    ZkVmProgramPerf,
};

fn prepare_input(
    cl_stf_proofs_with_vk: (Vec<ProofReceipt>, VerifyingKey),
) -> CheckpointProverInput {
    let (cl_stf_proofs, cl_stf_vk) = cl_stf_proofs_with_vk;
    CheckpointProverInput {
        cl_stf_proofs,
        cl_stf_vk,
    }
}

fn gen_perf_report(
    host: &impl ZkVmHostPerf,
    cl_stf_proofs_with_vk: (Vec<ProofReceipt>, VerifyingKey),
) -> PerformanceReport {
    let input = prepare_input(cl_stf_proofs_with_vk);
    CheckpointProgram::perf_report(&input, host).unwrap()
}

fn gen_proof(
    host: &impl ZkVmHost,
    cl_stf_proofs_with_vk: (Vec<ProofReceipt>, VerifyingKey),
) -> ProofReceipt {
    let input = prepare_input(cl_stf_proofs_with_vk);
    CheckpointProgram::prove(&input, host).unwrap()
}

#[cfg(feature = "sp1")]
pub mod sp1 {
    use strata_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
    use zkaleido::{VerifyingKey, ZkVmVerifier};
    use zkaleido_sp1_adapter::SP1Host;

    use super::*;
    use crate::programs::cl_stf;

    fn host() -> impl ZkVmHostPerf {
        SP1Host::init(&GUEST_EVM_EE_STF_ELF)
    }

    fn cl_stf_proofs_with_vk() -> (Vec<ProofReceipt>, VerifyingKey) {
        let evm_ee_proof = cl_stf::sp1::proof();
        let vk = cl_stf::sp1::vk();
        (vec![evm_ee_proof], vk)
    }

    pub fn perf_report() -> PerformanceReport {
        gen_perf_report(&host(), cl_stf_proofs_with_vk())
    }

    pub fn proof() -> ProofReceipt {
        gen_proof(&host(), cl_stf_proofs_with_vk())
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
    use crate::programs::cl_stf;

    fn host() -> impl ZkVmHostPerf {
        Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF)
    }

    fn cl_stf_proofs_with_vk() -> (Vec<ProofReceipt>, VerifyingKey) {
        let evm_ee_proof = cl_stf::risc0::proof();
        let vk = cl_stf::risc0::vk();
        (vec![evm_ee_proof], vk)
    }

    pub fn perf_report() -> PerformanceReport {
        gen_perf_report(&host(), cl_stf_proofs_with_vk())
    }

    pub fn proof() -> ProofReceipt {
        gen_proof(&host(), cl_stf_proofs_with_vk())
    }

    pub fn vk() -> VerifyingKey {
        host().vk()
    }
}
