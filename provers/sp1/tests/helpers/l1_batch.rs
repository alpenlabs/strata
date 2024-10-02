use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use alpen_test_utils::bitcoin::{get_btc_chain, get_tx_filters};
use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, params::MAINNET, Block};
use express_proofimpl_btc_blockspace::logic::BlockspaceProofOutput;
use express_proofimpl_l1_batch::L1BatchProofInput;
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
use express_sp1_guest_builder::{GUEST_BTC_BLOCKSPACE_ELF, GUEST_L1_BATCH_ELF};
use express_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
    ZKVMVerifier,
};
use sp1_sdk::{MockProver, Prover, SP1ProofWithPublicValues};

use super::{common::write_proof_to_file, get_btc_block_proof};
use crate::helpers::common::{read_proof_from_file, verify_proof_independently};

pub fn get_l1_batch_proof(start_height: u32, end_height: u32) -> Result<(Proof, VerificationKey)> {
    // Construct paths
    let l1_batch_name = format!("l1_batch_{}_{}.proof", start_height, end_height);
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let l1_batch_dir = manifest_dir.join("../../crates/test-utils/data/btc_proofs");
    if !l1_batch_dir.exists() {
        fs::create_dir(&l1_batch_dir).context("Failed to create 'l1_batch' directory")?;
    }

    let proof_file = l1_batch_dir.join(l1_batch_name);
    if proof_file.exists() {
        println!("Btc Batch proof found in cache, returning the cached proof..");
        let (proof, vk) = read_proof_from_file(&proof_file)?;
        verify_proof_independently(&proof, GUEST_L1_BATCH_ELF)?;
        return Ok((proof, vk));
    }

    let mut blockspace_outputs = Vec::new();
    let mut blockspace_proofs = Vec::new();
    let btc_chain = get_btc_chain();
    for height in start_height..end_height {
        let block = btc_chain.get_block(height);
        let (proof, vkey) = get_btc_block_proof(block).unwrap();
        let raw_output = SP1Verifier::extract_public_output::<Vec<u8>>(&proof)
            .expect("Failed to extract public outputs");
        let output: BlockspaceProofOutput = borsh::from_slice(&raw_output).unwrap();
        blockspace_outputs.push(output);
        blockspace_proofs.push(AggregationInput::new(proof, vkey));
    }

    let prover_options = ProverOptions {
        use_mock_prover: false,
        stark_to_snark_conversion: false,
        enable_compression: true,
    };
    let prover = SP1Host::init(GUEST_L1_BATCH_ELF.into(), prover_options);
    let input = L1BatchProofInput {
        batch: blockspace_outputs,
        state: btc_chain.get_verification_state(start_height, &MAINNET.clone().into()),
    };
    let mut l1_batch_input_builder = SP1ProofInputBuilder::new();
    l1_batch_input_builder.write_borsh(&input).unwrap();

    for proof in blockspace_proofs {
        l1_batch_input_builder.write_proof(proof).unwrap();
    }

    let l1_batch_input = l1_batch_input_builder.build().unwrap();

    let proof_res = prover
        .prove(l1_batch_input)
        .expect("Failed to generate proof");

    write_proof_to_file(&proof_res, &proof_file)?;

    Ok(proof_res)
}
