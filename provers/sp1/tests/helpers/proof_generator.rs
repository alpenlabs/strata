use std::{fs, path::PathBuf};

use anyhow::Result;
use sp1_sdk::{HashableKey, Prover, SP1VerifyingKey};
use strata_primitives::buf;
use strata_zkvm::{Proof, ProofType, VerificationKey, ZkVmHost, ZkVmProver};

pub trait ProofGenerator<T, P: ZkVmProver> {
    /// Generates a proof based on the input.
    fn get_input(&self, input: &T) -> Result<P::Input>;

    fn get_host(&self) -> impl ZkVmHost;

    /// Generates a proof based on the input.
    fn gen_proof(&self, input: &T) -> Result<(Proof, P::Output)>;

    /// Generates a unique proof ID based on the input.
    /// The proof ID will be the hash of the input and potentially other unique identifiers.
    fn get_proof_id(&self, input: &T) -> String;

    /// Retrieves a proof from cache or generates it if not found.
    fn get_proof(&self, input: &T) -> Result<(Proof, P::Output)> {
        let elf = self.get_elf();

        // 1. Create the unique proof ID
        let proof_id = format!(
            "{}_{}.proof",
            self.get_proof_id(input),
            short_program_id(elf),
        );
        println!("Getting proof for {}", proof_id);
        let proof_file = get_cache_dir().join(proof_id);

        // 2. Check if the proof file exists
        if proof_file.exists() {
            println!("Proof found in cache, returning the cached proof...",);
            let proof = read_proof_from_file(&proof_file)?;
            let host = self.get_host();
            verify_proof(&proof, host.get_verification_key())?;
            let output = P::process_output(&proof, &host)?;
            return Ok((proof, output));
        }

        // 3. Generate the proof
        println!("Proof not found in cache, generating proof...");
        let (proof, output) = self.gen_proof(input)?;

        // Verify the proof
        verify_proof(&proof, self.get_host().get_verification_key())?;

        // Save the proof to cache
        write_proof_to_file(&proof, &proof_file)?;

        Ok((proof, output))
    }

    /// Returns the ELF binary (used for verification).
    fn get_elf(&self) -> &[u8];

    // Simulate the proof. This is different than running the in the MOCK_PROVER mode
    // fn simulate(&self, input: T) -> U
}

/// Returns the cache directory for proofs.
fn get_cache_dir() -> std::path::PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("proofs")
}

/// Reads a proof from a file.
fn read_proof_from_file(proof_file: &std::path::Path) -> Result<Proof> {
    use std::{fs::File, io::Read};

    use anyhow::Context;

    let mut file = File::open(proof_file)
        .with_context(|| format!("Failed to open proof file {:?}", proof_file))?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .context("Failed to read proof file")?;

    Ok(Proof::new(buffer))
}

/// Writes a proof to a file.
fn write_proof_to_file(proof: &Proof, proof_file: &std::path::Path) -> Result<()> {
    use std::{fs::File, io::Write};

    use anyhow::Context;

    let cache_dir = get_cache_dir();
    if !cache_dir.exists() {
        fs::create_dir(&cache_dir).context("Failed to create 'proofs' directory")?;
    }

    let mut file = File::create(proof_file)
        .with_context(|| format!("Failed to create proof file {:?}", proof_file))?;

    file.write_all(proof.as_bytes())
        .context("Failed to write proof to file")?;

    Ok(())
}

/// Verifies a proof independently.
fn verify_proof(proof: &Proof, verifying_key: VerificationKey) -> Result<()> {
    use anyhow::Context;
    use sp1_sdk::{MockProver, SP1ProofWithPublicValues};

    let client = MockProver::new();
    let sp1_proof: SP1ProofWithPublicValues =
        bincode::deserialize(proof.as_bytes()).context("Failed to deserialize SP1 proof")?;
    let sp1_verifying_key: SP1VerifyingKey =
        bincode::deserialize(verifying_key.as_bytes()).expect("sp1 vk deser");
    client
        .verify(&sp1_proof, &sp1_verifying_key)
        .context("Independent proof verification failed")?;
    Ok(())
}

fn short_program_id(elf: &[u8]) -> String {
    use sp1_sdk::{MockProver, SP1ProofWithPublicValues};
    let client = MockProver::new();
    let (_, vk) = client.setup(elf);
    vk.bytes32().split_off(58)
}
