use std::{fs, path::PathBuf};

use anyhow::Result;
use sp1_sdk::{HashableKey, Prover, SP1Stdin};
use strata_zkvm::{ProofWithMetadata, ProverOptions, VerificationKey};
use tracing::{debug, info};

pub trait ProofGenerator<T> {
    /// Generates a proof based on the input.
    fn gen_proof(
        &self,
        input: &T,
        options: &ProverOptions,
    ) -> Result<(ProofWithMetadata, VerificationKey)>;

    /// Generates a unique proof ID based on the input.
    /// The proof ID will be the hash of the input and potentially other unique identifiers.
    fn get_proof_id(&self, input: &T) -> String;

    fn get_input(&self, input: &T) -> Result<SP1Stdin>;

    /// Retrieves a proof from cache or generates it if not found.
    fn get_proof(
        &self,
        input: &T,
        options: &ProverOptions,
    ) -> Result<(ProofWithMetadata, VerificationKey)> {
        debug!("Trying to get proof");
        let elf = self.get_elf();

        let proof_id = format!(
            "{}_{}.{}proof",
            self.get_proof_id(input),
            short_program_id(elf),
            options
        );
        info!(%proof_id, "Created the unique proof id");
        let proof_file = get_cache_dir().join(&proof_id);

        // 2. Check if the proof file exists
        if proof_file.exists() {
            info!(%proof_id, "Proof found in cache, reading the proof...");
            let proof_res = read_proof_from_file(&proof_file)?;
            verify_proof(&proof_res.0, elf)?;
            return Ok(proof_res);
        }

        // 3. Generate the proof
        info!(%proof_id, "Proof not found in cache, generating proof...");
        let proof_res = self.gen_proof(input, options)?;

        // Verify the proof
        verify_proof(&proof_res.0, self.get_elf())?;

        // Save the proof to cache
        write_proof_to_file(&proof_res, &proof_file)?;

        Ok(proof_res)
    }

    /// Returns the ELF binary (used for verification).
    fn get_elf(&self) -> &[u8];

    /// Simulate the proof. This is different than running the in the MOCK_PROVER mode
    #[allow(dead_code)] // FIXME:
    fn simulate(&self, input: &T) -> Result<()>;
}

/// Returns the cache directory for proofs.
fn get_cache_dir() -> std::path::PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("proofs")
}

/// Reads a proof from a file.
fn read_proof_from_file(
    proof_file: &std::path::Path,
) -> Result<(ProofWithMetadata, VerificationKey)> {
    use std::{fs::File, io::Read};

    use anyhow::Context;

    let mut file = File::open(proof_file)
        .with_context(|| format!("Failed to open proof file {:?}", proof_file))?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .context("Failed to read proof file")?;

    let proof = bincode::deserialize(&buffer).context("Failed to deserialize proof")?;
    info!(?proof_file, "Read proof to file");

    Ok(proof)
}

/// Writes a proof to a file.
fn write_proof_to_file(
    proof_res: &(ProofWithMetadata, VerificationKey),
    proof_file: &std::path::Path,
) -> Result<()> {
    use std::{fs::File, io::Write};

    use anyhow::Context;

    let serialized_proof =
        bincode::serialize(proof_res).context("Failed to serialize proof for writing")?;

    let cache_dir = get_cache_dir();
    if !cache_dir.exists() {
        fs::create_dir(&cache_dir).context("Failed to create 'proofs' directory")?;
    }

    let mut file = File::create(proof_file)
        .with_context(|| format!("Failed to create proof file {:?}", proof_file))?;

    file.write_all(&serialized_proof)
        .context("Failed to write proof to file")?;
    info!(?proof_file, "Saved proof to file");

    Ok(())
}

/// Verifies a proof independently.
fn verify_proof(proof: &ProofWithMetadata, elf: &[u8]) -> Result<()> {
    use anyhow::Context;
    use sp1_sdk::{MockProver, SP1ProofWithPublicValues};

    debug!("Verifying the proof");
    let client = MockProver::new();
    let sp1_proof: SP1ProofWithPublicValues =
        bincode::deserialize(proof.as_bytes()).context("Failed to deserialize SP1 proof")?;
    let (_, vk) = client.setup(elf);
    client
        .verify(&sp1_proof, &vk)
        .context("Independent proof verification failed")?;
    debug!("Verified the proof");
    Ok(())
}

fn short_program_id(elf: &[u8]) -> String {
    debug!("Generating short program id");
    use sp1_sdk::{MockProver, SP1ProofWithPublicValues};
    let client = MockProver::new();
    let (_, vk) = client.setup(elf);
    vk.bytes32().split_off(58)
}
