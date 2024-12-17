use borsh::{BorshDeserialize, BorshSerialize};
use serde::{de::DeserializeOwned, Serialize};

use crate::Proof;

/// A trait representing a Zero-Knowledge Virtual Machine (ZkVM) interface.
/// Provides methods for reading inputs, committing outputs, and verifying proofs
/// within the ZkVM environment.
pub trait ZkVmEnv {
    /// Reads a serialized byte buffer from the guest code.
    ///
    /// The input is expected to be written with [`write_buf`](crate::ZkVmInputBuilder::write_buf).
    fn read_buf(&self) -> Vec<u8>;

    /// Reads a serialized object from the guest code, deserializing it using Serde.
    ///
    /// The input is expected to be written with
    /// [`write_serde`](crate::ZkVmInputBuilder::write_serde).
    fn read_serde<T: DeserializeOwned>(&self) -> T;

    /// Reads a Borsh-serialized object from the guest code.
    ///
    /// The input is expected to be written with
    /// [`write_borsh`](`crate::ZkVmInputBuilder::write_borsh).
    fn read_borsh<T: BorshDeserialize>(&self) -> T {
        let buf = self.read_buf();
        borsh::from_slice(&buf).expect("borsh deserialization failed")
    }

    /// Commits a pre-serialized buffer to the public values stream.
    ///
    /// This method is intended for cases where the data has already been serialized
    /// outside of the ZkVM's standard serialization methods. It allows you to provide
    /// serialized outputs directly, bypassing any further serialization.
    fn commit_buf(&self, raw_output: &[u8]);

    /// Commits a Serde-serializable object to the public values stream.
    ///
    /// Values that are committed can be proven as public parameters.
    fn commit_serde<T: Serialize>(&self, output: &T);

    /// Commits a Borsh-serializable object to the public values stream.
    ///
    /// Values that are committed can be proven as public parameters.
    fn commit_borsh<T: BorshSerialize>(&self, output: &T) {
        self.commit_buf(&borsh::to_vec(output).expect("borsh serialization failed"));
    }

    /// Verifies a proof generated with the ZkVM.
    ///
    /// This method checks the validity of the proof against the provided verification key digest
    /// and public values. It will panic if the proof fails to verify.
    fn verify_native_proof(&self, vk_digest: &[u32; 8], public_values: &[u8]);

    /// Verifies a Groth16 proof.
    ///
    /// # Parameters
    ///
    /// * `proof`: [Proof](crate::Proof)
    /// * `verification_key`: A byte slice containing the serialized verification key.
    /// * `public_params_raw`: A byte slice containing the serialized public parameters.
    ///
    /// It will panic if the proof fails to verify.
    fn verify_groth16_proof(
        &self,
        proof: &Proof,
        verification_key: &[u8; 32],
        public_params_raw: &[u8],
    );

    /// Reads and verifies a committed output from another guest function.
    ///
    /// This is equivalent to calling [`ZkVmEnv::read_buf`] and [`ZkVmEnv::verify_native_proof`],
    /// but avoids double serialization and deserialization. The function will panic if the
    /// proof fails to verify.
    fn read_verified_buf(&self, vk_digest: &[u32; 8]) -> Vec<u8> {
        let public_values_raw = self.read_buf();
        self.verify_native_proof(vk_digest, &public_values_raw);
        public_values_raw
    }

    /// Reads and verifies a committed output from another guest function, deserializing it using
    /// Serde.
    ///
    /// This function is meant to read the committed output of another guest function
    /// that was written with [`ZkVmEnv::commit_serde`].
    /// It then verifies the proof against the given verification key digest.
    ///
    /// This is equivalent to calling [`ZkVmEnv::read_serde`] and [`ZkVmEnv::verify_native_proof`],
    /// but avoids double serialization and deserialization. The function will panic if the
    /// proof fails to verify.
    fn read_verified_serde<T: DeserializeOwned>(&self, vk_digest: &[u32; 8]) -> T;

    /// Reads and verifies a committed output from another guest function, deserializing it using
    /// Borsh.
    ///
    /// This function is similar to [`ZkVmEnv::read_verified_serde`], but is intended for guest
    /// commitments committed via [`ZkVmEnv::commit_borsh`]. The output is expected to be
    /// Borsh-serializable. It then verifies the proof using the internal verification key
    /// context.
    ///
    /// This is equivalent to calling [`ZkVmEnv::read_borsh`] and [`ZkVmEnv::verify_native_proof`],
    /// but avoids double serialization and deserialization. The function will panic if the
    /// proof fails to verify.
    fn read_verified_borsh<T: BorshDeserialize>(&self, vk_digest: &[u32; 8]) -> T {
        let verified_public_values_buf = self.read_verified_buf(vk_digest);
        borsh::from_slice(&verified_public_values_buf).expect("failed borsh deserialization")
    }
}
