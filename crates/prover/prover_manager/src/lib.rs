pub mod execution_env;

#[cfg(test)]
mod tests {
    use guest_builder::{GUEST_RETH_STF_ELF, GUEST_RETH_STF_ID};
    use risc0_adapter::{Risc0Verifier, RiscZeroHost};
    use zkvm::{ZKVMHost, ZKVMVerifier};
    use zkvm_primitives::{ELProofPublicParams, ZKVMInput};

    use crate::execution_env::el_proof_to_exec_update;

    const ENCODED_PROVER_INPUT: &[u8] = include_bytes!("../test_bin/1.bin");

    #[test]
    fn test_el_proving() {
        // constriction `ExecUpdate` from the proof
        let input: ZKVMInput = bincode::deserialize(ENCODED_PROVER_INPUT).unwrap();
        let prover = RiscZeroHost::init(GUEST_RETH_STF_ELF.into(), Default::default());

        let proof = prover.prove(input).expect("Failed to generate proof");

        // assert proof verification works
        Risc0Verifier::verify(GUEST_RETH_STF_ID, &proof).expect("Proof verification failed");

        let public_params: ELProofPublicParams =
            Risc0Verifier::extract_public_output(&proof).expect("Failed to extract public outputs");

        let exec_update = el_proof_to_exec_update(&public_params);
        println!("exec update {:?} ", exec_update);
    }
}
