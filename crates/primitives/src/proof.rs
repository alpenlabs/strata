use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{buf::Buf32, l1::L1BlockId, l2::L2BlockId};

#[derive(Clone, Debug, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollupVerifyingKey {
    // Verifying Key used to verify proof created using SP1
    #[serde(rename = "sp1")]
    SP1VerifyingKey(Buf32),
    // Verifying Key used to verify proof created using Risc0
    #[serde(rename = "risc0")]
    Risc0VerifyingKey(Buf32),
}

/// `ProofKey` is an enumeration representing various identifiers for proofs.
/// It is used to uniquely identify the proving tasks for different types of proofs.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    BorshSerialize,
    BorshDeserialize,
    Serialize,
    Deserialize,
)]
pub enum ProofKey {
    /// Represents the L1 block whose blockspace proof is being prepared.
    BtcBlockspace(L1BlockId),

    /// Represents a range of L1 blocks (inclusive) that are being proven as part of a batch.
    L1Batch(L1BlockId, L1BlockId),

    /// Represents EVM Execution Environment (EE) block for which the State Transition Function
    /// (STF) proof is being generated.
    EvmEeStf(Buf32),

    /// Represents the height of the Consensus Layer (CL) block for which the State Transition
    /// Function (STF) proof is being generated.
    ClStf(L2BlockId),

    /// Represents the range of Consensus Layer (CL) blocks for which are aggregated
    ClAgg(L2BlockId, L2BlockId),

    /// Represents the index of the checkpoint that is being proven.
    Checkpoint(u64),
}
