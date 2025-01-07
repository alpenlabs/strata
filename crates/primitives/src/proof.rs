use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::{buf::Buf32, l1::L1BlockId, l2::L2BlockId};

/// Represents the verifying key used for verifying ZK proofs in a rollup context.
///
/// This enum encapsulates verifying keys for different ZKVMs:
/// - `SP1VerifyingKey`: Used for verifying proofs generated using SP1.
/// - `Risc0VerifyingKey`: Used for verifying proofs generated using Risc0.
/// - `Native`: For functional testing purposes without ZKVM overhead.
#[derive(Clone, Debug, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollupVerifyingKey {
    /// Verifying Key for proofs generated by SP1.
    #[serde(rename = "sp1")]
    SP1VerifyingKey(Buf32),

    /// Verifying Key for proofs generated by Risc0.
    #[serde(rename = "risc0")]
    Risc0VerifyingKey(Buf32),

    /// Placeholder variant for functional testing.
    ///
    /// This variant allows skipping guest code compilation (e.g., ELFs for SP1 or Risc0) and is
    /// used to test the prover-client and proof logic without the overhead of ZKVM
    /// compilation. It is strictly for internal testing and must not be used in production
    /// deployments.
    #[serde(rename = "native")]
    NativeVerifyingKey(Buf32),
}

/// Represents a context for different types of proofs.
///
/// This enum categorizes proofs by their associated context, including the type of proof and its
/// range or scope. Each variant includes relevant metadata required to distinguish and track the
/// proof.
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
pub enum ProofContext {
    /// Identifier representing a Bitcoin L1 block for blockscan proof
    BtcBlockspace(L1BlockId),

    /// Identifier for a batch of L1 blocks being proven.
    /// Includes the starting and ending block heights.
    L1Batch(L1BlockId, L1BlockId),

    /// Identifier for the EVM Execution Environment (EE) blocks used in generating the State
    /// Transition Function (STF) proof.
    EvmEeStf(Buf32, Buf32),

    /// Identifier for the Consensus Layer (CL) blocks used in generating the State Transition
    /// Function (STF) proof.
    ClStf(L2BlockId, L2BlockId),

    /// Identifier for a batch of Consensus Layer (CL) blocks being proven.
    /// Includes the starting and ending block heights.
    ClAgg(L2BlockId, L2BlockId),

    /// Identifier for a specific checkpoint being proven.
    Checkpoint(u64),
}

/// Represents the ZkVm host used for proof generation.
///
/// This enum identifies the ZkVm environment utilized to create a proof.
/// Available hosts:
/// - `SP1`: SP1 ZKVM.
/// - `Risc0`: Risc0 ZKVM.
/// - `Native`: Native ZKVM.
#[non_exhaustive]
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
pub enum ProofZkVm {
    SP1,
    Risc0,
    Native,
}

/// Represents a unique key for identifying any type of proof.
///
/// A `ProofKey` combines a `ProofContext` (which specifies the type of proof and its scope)
/// with a `ProofZkVm` (which specifies the ZKVM host used for proof generation).
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
pub struct ProofKey {
    /// The unique identifier for the proof type and its context.
    context: ProofContext,
    /// The ZKVM host used for proof generation.
    host: ProofZkVm,
}

impl ProofKey {
    pub fn new(context: ProofContext, host: ProofZkVm) -> Self {
        Self { context, host }
    }

    pub fn context(&self) -> &ProofContext {
        &self.context
    }

    pub fn host(&self) -> &ProofZkVm {
        &self.host
    }
}
