use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use alpen_test_utils::bitcoin::get_tx_filters;
use anyhow::{Context, Result};
use bitcoin::{consensus::serialize, Block};
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder, SP1Verifier};
use express_sp1_guest_builder::GUEST_BTC_BLOCKSPACE_ELF;
use express_zkvm::{
    Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder, ZKVMVerifier,
};
use sp1_sdk::{MockProver, Prover, SP1ProofWithPublicValues};

use crate::helpers::common::{
    read_proof_from_file, verify_proof_independently, write_proof_to_file,
};

pub fn get_btc_block_proof(block: &Block) -> Result<(Proof, VerificationKey)> {
    // Construct paths
    let block_hash = format!("block_{}.proof", block.block_hash());
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let btc_proofs_dir = manifest_dir.join("../../crates/test-utils/data/btc_proofs");
    if !btc_proofs_dir.exists() {
        fs::create_dir(&btc_proofs_dir).context("Failed to create 'btc_proofs' directory")?;
    }

    let proof_file = btc_proofs_dir.join(block_hash);
    if proof_file.exists() {
        println!("Btc Proof found in cache, returning the cached proof..");
        let (proof, vk) = read_proof_from_file(&proof_file)?;
        verify_proof_independently(&proof, GUEST_BTC_BLOCKSPACE_ELF)?;
        return Ok((proof, vk));
    }

    let filters = get_tx_filters();
    let prover_options = ProverOptions {
        use_mock_prover: false,
        stark_to_snark_conversion: false,
        enable_compression: true,
    };
    let prover = SP1Host::init(GUEST_BTC_BLOCKSPACE_ELF.into(), prover_options);

    let serialized_block = serialize(&block);

    let input = SP1ProofInputBuilder::new()
        .write_borsh(&filters)
        .unwrap()
        .write_serialized(&serialized_block)
        .unwrap()
        .build()
        .unwrap();

    let proof_res = prover.prove(input).expect("Failed to generate proof");

    write_proof_to_file(&proof_res, &proof_file)?;

    Ok(proof_res)
}
