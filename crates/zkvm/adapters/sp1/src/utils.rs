use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::Context;
use sha2::{Digest, Sha256};
use sp1_sdk::{ProverClient, SP1ProofWithPublicValues, SP1ProvingKey, SP1VerifyingKey};
use strata_zkvm::Proof;
use tracing::{debug, error};

/// Generates or retrieves proving and verifying keys for the given guest code.
///
/// This function may use caching to avoid the expensive operation of key generation.
/// If `use_cache` is true, it will attempt to load the keys from the cache;
/// otherwise, it will generate new keys.
pub fn get_proving_keys(
    client: &ProverClient,
    guest_code: &Vec<u8>,
    use_cache: bool,
) -> (SP1ProvingKey, SP1VerifyingKey) {
    if use_cache {
        // Compute the SHA-256 hash of the guest_code
        let mut hasher = Sha256::new();
        hasher.update(guest_code);
        let proving_key_file_name = format!("{:x}", hasher.finalize());

        // Define the cache directory and file path
        let cache_dir = PathBuf::from("proving_keys");

        // Create cache directory if it doesn't exist
        let _ = fs::create_dir_all(&cache_dir); // Ignore errors

        let cache_file_path = cache_dir.join(format!("{}.bin", proving_key_file_name));

        // Attempt to read from cache
        if let Ok(mut file) = File::open(&cache_file_path) {
            let mut buffer = Vec::new();
            if file.read_to_end(&mut buffer).is_ok() {
                if let Ok(keys) = bincode::deserialize(&buffer) {
                    debug!(
                        "Reading the proving key from cache {:?}",
                        proving_key_file_name
                    );
                    return keys;
                }
            }
            // If any error occurs, fall back to generating keys
        }

        // Generate keys using client.setup
        debug!(
            "Proving key '{:?}' not found in cache generating new...",
            proving_key_file_name
        );
        let keys = client.setup(guest_code);

        // Attempt to save to cache
        if let Ok(encoded) = bincode::serialize(&keys) {
            if let Ok(mut file) = File::create(&cache_file_path) {
                let _ = file.write_all(&encoded);
                error!(
                    "Failed to write the Proving key {:?}",
                    proving_key_file_name
                );
                // Ignore errors when writing to cache
            }
        }

        keys
    } else {
        // If caching is not used, directly generate keys
        client.setup(guest_code)
    }
}

/// Extracts the raw Groth16 proof bytes from a given [`Proof`].
pub fn extract_raw_groth16_proof(proof: Proof) -> anyhow::Result<Proof> {
    let sp1_proof: SP1ProofWithPublicValues = bincode::deserialize(proof.as_bytes())
        .context("Failed to deserialize SP1 Groth16 proof")?;

    Ok(Proof::new(sp1_proof.bytes()))
}
