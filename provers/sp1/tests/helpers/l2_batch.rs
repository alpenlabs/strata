#![allow(dead_code)]
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use express_sp1_guest_builder::{GUEST_CL_AGG_ELF, GUEST_CL_STF_ELF};
use express_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
};
use sp1_sdk::{HashableKey, MockProver, Prover, SP1ProofWithPublicValues};

use crate::helpers::{
    common::{read_proof_from_file, verify_proof_independently, write_proof_to_file},
    get_cl_stf_proof, get_el_block_proof,
};

pub fn get_cl_batch_proof(start_height: u32, end_height: u32) -> Result<(Proof, VerificationKey)> {
    // Construct paths
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cl_witness_path =
        manifest_dir.join(format!("../test-util/cl/cl_witness_{}.bin", start_height));
    println!("Looking for the CL witness path: {:?}", cl_witness_path);

    let cl_batch_proofs_dir = cl_witness_path
        .parent()
        .context("Failed to get parent directory of CL witness path")?
        .join("cl_proofs");

    if !cl_batch_proofs_dir.exists() {
        fs::create_dir(&cl_batch_proofs_dir).context("Failed to create 'cl_proofs' directory")?;
    }

    let proof_file = cl_batch_proofs_dir.join(format!("proof_{}_{}.bin", start_height, end_height));

    if proof_file.exists() {
        println!("CL Batch Proof found in cache, returning the cached proof...");
        let (proof, vk) = read_proof_from_file(&proof_file)?;
        verify_proof_independently(&proof, GUEST_CL_AGG_ELF)?;
        return Ok((proof, vk));
    }

    println!("CL Proof not found in cache, generating the proof...");
    let proof_res = generate_proof(start_height, end_height)?;

    write_proof_to_file(&proof_res, &proof_file)?;

    Ok(proof_res)
}

fn generate_proof(start_height: u32, end_height: u32) -> Result<(Proof, VerificationKey)> {
    let prover_ops = ProverOptions {
        enable_compression: true,
        use_mock_prover: false,
        ..Default::default()
    };
    let prover = SP1Host::init(GUEST_CL_AGG_ELF.into(), prover_ops);

    let mut agg_proof_inputs: Vec<AggregationInput> = Vec::new();
    for block in start_height..end_height {
        let (proof, vk) = get_cl_stf_proof(block).unwrap();
        println!("gen proof cl");
        agg_proof_inputs.push(AggregationInput::new(proof, vk));
    }

    let mut prover_input_builder = SP1ProofInputBuilder::new();
    let len = (end_height - start_height) as usize;
    println!("start - end {}-{}", start_height, end_height);
    println!("len {}", len);
    prover_input_builder.write(&len).unwrap();
    for agg_proof in agg_proof_inputs {
        prover_input_builder.write_proof(agg_proof).unwrap();
    }

    let prover_input = prover_input_builder.build().unwrap();
    let proof = prover
        .prove(prover_input)
        .context("Failed to generate proof")?;

    Ok(proof)
}
