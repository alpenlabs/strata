use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::l1::{BitcoinAmount, OutputRef};

use crate::{batch::SignedBatchCheckpoint, da_blob::PayloadCommitment};

/// Information related to relevant transactions to be stored in L1Tx
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
#[allow(clippy::large_enum_variant)]
pub enum ProtocolOperation {
    /// Deposit Transaction
    Deposit(DepositInfo),
    /// Deposit Request info
    DepositRequest(DepositRequestInfo),
    /// Checkpoint data
    Checkpoint(SignedBatchCheckpoint),
    /// DA data. can be made through `submit_da_blob` RPC.
    DA(PayloadCommitment),
    // TODO: add other kinds like statediffs
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct DepositInfo {
    /// Bitcoin amount
    pub amt: BitcoinAmount,

    /// outpoint
    pub outpoint: OutputRef,

    /// EE address
    pub address: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct DepositRequestInfo {
    /// amount in satoshis
    pub amt: u64,

    /// tapscript control block hash for timelock script
    pub take_back_leaf_hash: [u8; 32],

    /// EE address
    pub address: Vec<u8>,
}

/// Wrapper to hold various types of Envelope data as defined by [`PayloadTypeTag`] enum.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct EnvelopePayload {
    /// for tagging purpose to understand what kind of envelope it is
    tag: PayloadTypeTag,
    /// payload present in envelope
    data: Vec<u8>,
}

impl EnvelopePayload {
    pub fn new(data_type: PayloadTypeTag, data: Vec<u8>) -> Self {
        Self {
            tag: data_type,
            data,
        }
    }

    /// Raw payload bytes
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Envelope type
    pub fn tag(&self) -> PayloadTypeTag {
        self.tag
    }
}

/// Enum that acts as a tag to separates different types of envelope blobs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum PayloadTypeTag {
    Checkpoint,
    DA,
}
