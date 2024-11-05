mod helpers;
// NOTE: SP1 prover runs in release mode only; therefore run the tests on release mode only
#[cfg(all(feature = "prover", not(debug_assertions)))]
mod test {
    use strata_proofimpl_evm_ee_stf::ELProofPublicParams;
    use strata_sp1_adapter::SP1Verifier;
    use strata_zkvm::{ProverOptions, ZkVmVerifier};

    use crate::helpers::{ElProofGenerator, ProofGenerator};

    #[test]
    fn test_reth_stf_guest_code_trace_generation() {
        sp1_sdk::utils::setup_logger();
        let height = 1;

        let el_prover = ElProofGenerator::new();

        let _ = el_prover.get_proof(&height).unwrap();
    }
}
