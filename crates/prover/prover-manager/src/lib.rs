#[cfg(test)]
mod tests {
    use guest_builder::{GUEST_RETH_STF_ELF, GUEST_RETH_STF_ID};
    use risc0_adapter::{Risc0Verifier, RiscZeroHost};
    use zkvm::{ZKVMHost, ZKVMVerifier};
    use zkvm_primitives::{ELProofPublicParams, ZKVMInput};

    use alpen_vertex_evmexec::el_payload::ElPayload;
    use alpen_vertex_primitives::{buf::Buf20, prelude::Buf32};
    use alpen_vertex_state::exec_update::{ExecUpdate, UpdateInput, UpdateOutput};

    const ENCODED_PROVER_INPUT: &[u8] = include_bytes!("../test_bin/1.bin");

    fn get_mock_exec_update(public_parmas: ELProofPublicParams) -> ExecUpdate {
        let mock_el_payload = ElPayload {
            base_fee_per_gas: Buf32::zero(),
            block_hash: Buf32(public_parmas.new_blockhash),
            block_number: public_parmas.block_idx,
            extra_data: Vec::new(),
            fee_recipient: Buf20::zero(),
            gas_limit: 0,
            gas_used: 0,
            logs_bloom: [0; 256],
            parent_hash: Buf32(public_parmas.prev_blockhash),
            prev_randao: Buf32::zero(),
            state_root: Buf32(public_parmas.new_state_root),
            timestamp: 0,
            transactions: Vec::new(),
            receipts_root: Buf32::zero(),
        };

        let payload_vec = borsh::to_vec(&mock_el_payload).unwrap();
        let update_input = UpdateInput::new(
            public_parmas.block_idx,
            Buf32(public_parmas.txn_root),
            payload_vec,
        );
        let update_output = UpdateOutput::new_from_state(Buf32(public_parmas.new_state_root));

        ExecUpdate::new(update_input, update_output)
    }

    fn exec_update_to_el_proof_pp(exec_update: ExecUpdate) -> ELProofPublicParams {
        let exec_input = exec_update.input();
        let exec_output = exec_update.output();
        let payload = borsh::from_slice::<ElPayload>(exec_input.extra_payload()).unwrap();

        ELProofPublicParams {
            block_idx: exec_input.update_idx(),
            prev_blockhash: payload.parent_hash.0,
            new_blockhash: payload.block_hash.0,
            new_state_root: exec_output.new_state().0,
            txn_root: exec_input.entries_root().0,
            withdrawals: Default::default(),
        }
    }

    #[test]
    fn test_el_proving() {
        let input: ZKVMInput = bincode::deserialize(ENCODED_PROVER_INPUT).unwrap();
        let prover = RiscZeroHost::init(GUEST_RETH_STF_ELF.into(), Default::default());

        let proof = prover
            .prove(input.clone())
            .expect("Failed to generate proof");

        let public_params: ELProofPublicParams =
            Risc0Verifier::extract_public_output(&proof).expect("Failed to extract public outputs");

        let exec_update = get_mock_exec_update(public_params.clone());
        let public_param = exec_update_to_el_proof_pp(exec_update);

        Risc0Verifier::verify_with_public_params(GUEST_RETH_STF_ID, public_param, &proof)
            .expect("Proof verification failed");
    }
}
