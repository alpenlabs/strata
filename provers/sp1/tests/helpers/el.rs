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
use sp1_sdk::{MockProver, Prover, SP1ProofWithPublicValues};

use crate::helpers::common::{
    read_proof_from_file, verify_proof_independently, write_proof_to_file,
};

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
        let (proof, vk) = read_proof_from_file(&proof_file)?;
        verify_proof_independently(&proof, GUEST_EVM_EE_STF_ELF)?;
        return Ok((proof, vk));
    }

    println!("Proof not found in cache, generating the proof ...");
    let proof_res = generate_proof(el_proof_input)?;

    write_proof_to_file(&proof_res, &proof_file)?;

    Ok(proof_res)
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
