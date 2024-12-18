use std::{fs, path::PathBuf};

use strata_zkvm::{ProofReceipt, ProofReport, ZkVmHost, ZkVmProofError, ZkVmProver, ZkVmResult};
pub mod btc;
mod checkpoint;
pub mod cl;
pub mod el;
mod generators;
pub mod l1_batch;
pub mod l2_batch;

pub use generators::TEST_NATIVE_GENERATORS;
#[cfg(feature = "risc0")]
pub use generators::TEST_RISC0_GENERATORS;
#[cfg(feature = "sp1")]
pub use generators::TEST_SP1_GENERATORS;

pub trait ProofGenerator<P: ZkVmProver> {
    type Input;

    /// An input required to generate a proof.
    fn get_input(&self, input: &Self::Input) -> ZkVmResult<P::Input>;

    // A host to generate the proof against.
    fn get_host(&self) -> impl ZkVmHost;

    /// Generates a unique proof ID based on the input.
    /// The proof ID will be the hash of the input and potentially other unique identifiers.
    fn get_proof_id(&self, input: &Self::Input) -> String;

    /// Retrieves a proof from cache or generates it if not found.
    fn get_proof(&self, input: &Self::Input) -> ZkVmResult<ProofReceipt> {
        // 1. Create the unique proof ID
        let proof_id = format!("{}_{}.proof", self.get_proof_id(input), self.get_host());
        println!("Getting proof for {}", proof_id);
        let proof_file = get_cache_dir().join(proof_id);

        // 2. Check if the proof file exists
        if proof_file.exists() {
            println!("Proof found in cache, returning the cached proof...",);
            let proof = read_proof_from_file(&proof_file)?;
            let host = self.get_host();
            verify_proof(&proof, &host)?;
            return Ok(proof);
        }

        // 3. Generate the proof
        println!("Proof not found in cache, generating proof...");
        let proof = self.gen_proof(input)?;

        // Verify the proof
        verify_proof(&proof, &self.get_host())?;

        // Save the proof to cache
        write_proof_to_file(&proof, &proof_file).unwrap();

        Ok(proof)
    }

    /// Generates a proof based on the input.
    fn gen_proof(&self, input: &Self::Input) -> ZkVmResult<ProofReceipt> {
        let input = self.get_input(input)?;
        let host = self.get_host();
        <P as ZkVmProver>::prove(&input, &host)
    }

    /// Generates a proof report based on the input.
    fn gen_perf_report(&self, input: &Self::Input) -> ZkVmResult<ProofReport> {
        let input = self.get_input(input)?;
        let host = self.get_host();
        <P as ZkVmProver>::perf_stats(&input, &host)
    }
}

/// Returns the cache directory for proofs.
fn get_cache_dir() -> std::path::PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("proofs")
}

/// Reads a proof from a file.
fn read_proof_from_file(proof_file: &std::path::Path) -> Result<ProofReceipt, ZkVmProofError> {
    use std::{fs::File, io::Read};

    let mut file = File::open(proof_file).expect("Failed to open proof file");

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .expect("Failed to read proof file");
    let proof_receipt: ProofReceipt = bincode::deserialize(&buffer)?;

    Ok(proof_receipt)
}

/// Writes a proof to a file.
fn write_proof_to_file(proof: &ProofReceipt, proof_file: &std::path::Path) -> Result<(), String> {
    use std::{fs::File, io::Write};

    let cache_dir = get_cache_dir();
    if !cache_dir.exists() {
        fs::create_dir(&cache_dir).expect("Failed to create 'proofs' directory");
    }

    let mut file = File::create(proof_file).expect("Failed to create proof file");

    file.write_all(&bincode::serialize(&proof).expect("serialization of proof failed"))
        .expect("Failed to write proof to file");

    Ok(())
}

/// Verifies a proof independently.
fn verify_proof(proof: &ProofReceipt, host: &impl ZkVmHost) -> ZkVmResult<()> {
    host.verify(proof)
}
