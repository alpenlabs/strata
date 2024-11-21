use std::path::PathBuf;

use anyhow::Result;
use strata_zkvm::{Proof, ProofWithInfo, ZkVmHost, ZkVmProver};

pub trait ProofGenerator<T, P: ZkVmProver> {
    /// Generates a proof based on the input.
    fn get_input(&self, input: &T) -> Result<P::Input>;

    fn get_host(&self) -> impl ZkVmHost;

    /// Generates a proof based on the input.
    fn gen_proof(&self, input: &T) -> Result<ProofWithInfo>;

    /// Generates a unique proof ID based on the input.
    /// The proof ID will be the hash of the input and potentially other unique identifiers.
    fn get_proof_id(&self, input: &T) -> String;

    /// Retrieves a proof from cache or generates it if not found.
    fn get_proof(&self, input: &T) -> Result<ProofWithInfo> {
        // 1. Create the unique proof ID
        let proof_id = format!("{}_{}.proof", self.get_proof_id(input), self.get_host());
        println!("Getting proof for {}", proof_id);
        let proof_file = get_cache_dir().join(proof_id);

        // 2. Check if the proof file exists
        let proof = if proof_file.exists() {
            println!("Proof found in cache, returning the cached proof...",);
            ProofWithInfo::load(&proof_file)?
        } else {
            // 3. Generate the proof
            println!("Proof not found in cache, generating proof...");
            self.gen_proof(input)?
        };

        // Verify the proof
        verify_proof(proof.proof(), &self.get_host())?;

        proof.save(proof_file)?;

        Ok(proof)
    }

    // Simulate the proof. This is different than running the in the MOCK_PROVER mode
    // fn simulate(&self, input: T) -> U
}

/// Returns the cache directory for proofs.
fn get_cache_dir() -> std::path::PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("proofs")
}

/// Verifies a proof independently.
fn verify_proof(proof: &Proof, host: &impl ZkVmHost) -> Result<()> {
    host.verify(proof)
}
