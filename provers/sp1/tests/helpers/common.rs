use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

use anyhow::{Context, Result};
use express_zkvm::{Proof, VerificationKey};
use sp1_sdk::{MockProver, Prover, SP1ProofWithPublicValues};

/// Reads a proof and its verification key from a file.
pub fn read_proof_from_file(proof_file: &Path) -> Result<(Proof, VerificationKey)> {
    let mut file = File::open(proof_file)
        .with_context(|| format!("Failed to open proof file {:?}", proof_file))?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .context("Failed to read proof file")?;

    let proof = bincode::deserialize(&buffer).context("Failed to deserialize proof")?;
    Ok(proof)
}

/// Writes a proof and its verification key to a file.
pub fn write_proof_to_file(proof_res: &(Proof, VerificationKey), proof_file: &Path) -> Result<()> {
    let serialized_proof =
        bincode::serialize(proof_res).context("Failed to serialize proof for writing")?;

    let mut file = File::create(proof_file)
        .with_context(|| format!("Failed to create proof file {:?}", proof_file))?;

    file.write_all(&serialized_proof)
        .context("Failed to write proof to file")?;

    Ok(())
}

/// Verifies a proof independently using the mock prover.
pub fn verify_proof_independently(proof: &Proof, elf: &[u8]) -> Result<()> {
    let client = MockProver::new();
    let sp1_proof: SP1ProofWithPublicValues =
        bincode::deserialize(proof.as_bytes()).context("Failed to deserialize SP1 proof")?;
    let (_, vk) = client.setup(elf);
    client
        .verify(&sp1_proof, &vk)
        .context("Independent proof verification failed")
}
