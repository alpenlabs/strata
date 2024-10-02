#![allow(dead_code)]
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use express_proofimpl_evm_ee_stf::ELProofInput;
use express_sp1_adapter::{SP1Host, SP1ProofInputBuilder};
use express_sp1_guest_builder::GUEST_EVM_EE_STF_ELF;
use express_zkvm::{Proof, ProverOptions, VerificationKey, ZKVMHost, ZKVMInputBuilder};

pub fn get_el_block_proof(witness_path: &Path) -> Result<(Proof, VerificationKey)> {
    let json_file = fs::read_to_string(witness_path)
        .with_context(|| format!("Failed to read JSON file at {:?}", witness_path))?;
    let el_proof_input: ELProofInput =
        serde_json::from_str(&json_file).context("Failed to parse JSON into ELProofInput")?;
    let block_num = el_proof_input.parent_header.number + 1;
    let witness_dir = witness_path
        .parent()
        .context("Witness path has no parent directory")?;
    let proofs_dir: PathBuf = witness_dir.join("el_proofs");
    if !proofs_dir.exists() {
        fs::create_dir_all(&proofs_dir)
            .context("Failed to create el_proofs directory inside witness folder")?;
    }
    let proof_file = proofs_dir.join(format!("proof_{}.bin", block_num));
    if proof_file.exists() {
        println!("Proof found in cache, returning the cached proof ...");
        return read_proof_from_file(&proof_file);
    }

    println!("Proof not found in cache, generating the proof ...");
    let proof_res = generate_proof(el_proof_input)?;
    write_proof_to_file(&proof_res, &proof_file)?;

    Ok(proof_res)
}

fn read_proof_from_file(proof_file: &Path) -> Result<(Proof, VerificationKey)> {
    let mut file = fs::File::open(proof_file)
        .with_context(|| format!("Failed to open existing proof file at {:?}", proof_file))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .context("Failed to read proof file")?;
    let proof = bincode::deserialize(&buffer).context("Failed to deserialize proof")?;
    Ok(proof)
}

fn write_proof_to_file(proof_res: &(Proof, VerificationKey), proof_file: &Path) -> Result<()> {
    let serialized_proof = bincode::serialize(proof_res).context("Failed to serialize proof")?;
    let mut file = fs::File::create(proof_file)
        .with_context(|| format!("Failed to create proof file at {:?}", proof_file))?;
    file.write_all(&serialized_proof)
        .context("Failed to write proof to file")?;
    Ok(())
}

fn generate_proof(el_proof_input: ELProofInput) -> Result<(Proof, VerificationKey)> {
    let prover_ops = ProverOptions {
        use_mock_prover: false,
        enable_compression: true,
        ..Default::default()
    };
    let prover = SP1Host::init(GUEST_EVM_EE_STF_ELF.into(), prover_ops);
    let proof_input = SP1ProofInputBuilder::new()
        .write(&el_proof_input)
        .context("Failed to write el_proof_input")?
        .build()
        .context("Failed to build proof_input")?;
    let proof = prover
        .prove(proof_input)
        .context("Failed to generate proof")?;
    Ok(proof)
}
