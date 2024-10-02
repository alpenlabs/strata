#![allow(dead_code)]
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use express_sp1_guest_builder::GUEST_CL_STF_ELF;
use express_zkvm::{
    AggregationInput, Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder,
};

use crate::helpers::get_el_block_proof;

pub fn get_cl_stf_proof(block_num: u32) -> Result<(Proof, VerificationKey)> {
    // Construct paths
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cl_witness_path =
        manifest_dir.join(format!("../test-util/cl/cl_witness_{}.bin", block_num));
    println!("Looking for the CL witness path: {:?}", cl_witness_path);

    let cl_proofs_dir = cl_witness_path
        .parent()
        .context("Failed to get parent directory of CL witness path")?
        .join("cl_proofs");

    if !cl_proofs_dir.exists() {
        fs::create_dir(&cl_proofs_dir).context("Failed to create 'cl_proofs' directory")?;
    }

    let proof_file = cl_proofs_dir.join(format!("proof_{}.bin", block_num));

    if proof_file.exists() {
        println!("CL Proof found in cache, returning the cached proof...");
        return read_proof_from_file(&proof_file);
    }

    // Prepare the CL Witness and EL Proofs
    let el_witness_path = manifest_dir.join(format!("../test-util/el/witness_{}.json", block_num));
    let (el_proof, vk) = get_el_block_proof(&el_witness_path)?;

    let agg_input = AggregationInput::new(el_proof, vk);

    let cl_witness = fs::read(&cl_witness_path)
        .with_context(|| format!("Failed to read CL witness file {:?}", cl_witness_path))?;

    println!("CL Proof not found in cache, generating the proof...");
    let proof_res = generate_proof(cl_witness, agg_input)?;

    write_proof_to_file(&proof_res, &proof_file)?;

    Ok(proof_res)
}

fn read_proof_from_file(proof_file: &Path) -> Result<(Proof, VerificationKey)> {
    let mut file = File::open(proof_file)
        .with_context(|| format!("Failed to open proof file {:?}", proof_file))?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .context("Failed to read proof file")?;

    let proof = bincode::deserialize(&buffer).context("Failed to deserialize proof")?;

    Ok(proof)
}

fn write_proof_to_file(proof_res: &(Proof, VerificationKey), proof_file: &Path) -> Result<()> {
    let serialized_proof =
        bincode::serialize(proof_res).context("Failed to serialize proof for writing")?;

    let mut file = File::create(proof_file)
        .with_context(|| format!("Failed to create proof file {:?}", proof_file))?;

    file.write_all(&serialized_proof)
        .context("Failed to write proof to file")?;

    Ok(())
}

fn generate_proof(
    cl_raw_witness: Vec<u8>,
    agg_proof: AggregationInput,
) -> Result<(Proof, VerificationKey)> {
    let prover_ops = ProverOptions {
        enable_compression: true,
        use_mock_prover: false,
        ..Default::default()
    };
    let prover = SP1Host::init(GUEST_CL_STF_ELF.into(), prover_ops);

    let proof_input = SP1ProofInputBuilder::new()
        .write_proof(agg_proof)?
        .write(&cl_raw_witness)?
        .build()?;

    let proof = prover
        .prove(proof_input)
        .context("Failed to generate proof")?;

    Ok(proof)
}
