use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::buf::Buf32;

#[derive(Clone, Debug, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollupVerifyingKey {
    // Verifying Key used to verify proof created using SP1
    SP1VerifyingKey(Buf32),
    // Verifying Key used to verify proof created using Risc0
    Risc0VerifyingKey(Buf32),
}

/// `ProofId` is an enumeration representing various identifiers for proofs.
/// It is used to uniquely identify the proofing tasks for different types of proofs.
#[derive(
    Debug, Clone, Copy, PartialEq, BorshSerialize, BorshDeserialize, Serialize, Deserialize,
)]
pub enum ProofId {
    /// Represents the height of the L1 block whose blockspace proof is being prepared.
    BtcBlockspace(u64),

    /// Represents a range of L1 blocks (inclusive) that are being proven as part of a batch.
    /// The first `u64` is the starting height, and the second `u64` is the ending height.
    L1Batch(u64, u64),

    /// Represents the height of the EVM Execution Environment (EE) block for which
    /// the State Transition Function (STF) proof is being generated.
    EvmEeStf(u64),

    /// Represents the height of the Consensus Layer (CL) block for which
    /// the State Transition Function (STF) proof is being generated.
    ClStf(u64),

    /// Represents the index of the checkpoint that is being proven.
    Checkpoint(u64),
}
