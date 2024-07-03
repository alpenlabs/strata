use std::{
    fmt::{self, Debug},
    hash::BuildHasherDefault,
};

use alpen_vertex_primitives::l1::L1Tx;
use alpen_vertex_primitives::prelude::*;
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::l1::L1HeaderPayload;

/// ID of an L2 block, usually the hash of its root header.
#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Arbitrary, BorshSerialize, BorshDeserialize,
)]
pub struct L2BlockId(Buf32);

impl From<Buf32> for L2BlockId {
    fn from(value: Buf32) -> Self {
        Self(value)
    }
}

impl From<L2BlockId> for Buf32 {
    fn from(value: L2BlockId) -> Self {
        value.0
    }
}

impl fmt::Debug for L2BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

/// Full contents of the bare L2 block.
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L2Block {
    /// Header that links the block into the L2 block chain and carries the
    /// block's credential from a sequencer.
    header: L2BlockHeader,

    /// Body that contains the bulk of the data.
    body: L2BlockBody,
}

impl L2Block {
    pub fn new(header: L2BlockHeader, body: L2BlockBody) -> Self {
        Self { header, body }
    }

    pub fn header(&self) -> &L2BlockHeader {
        &self.header
    }

    pub fn l1_segment(&self) -> &L1Segment {
        &self.body.l1_segment
    }

    pub fn exec_segment(&self) -> &ExecSegment {
        &self.body.exec_segment
    }
}

/// Block header that forms the chain we use to reach consensus.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Arbitrary)]
pub struct L2BlockHeader {
    /// Block index, obviously.
    pub(crate) block_idx: u64,

    /// Timestamp the block was (intended to be) published at.
    pub(crate) timestamp: u64,

    /// Hash of the previous block, to form the blockchain.
    pub(crate) prev_block: L2BlockId,

    /// Hash of the L1 segment.
    pub(crate) l1_segment_hash: Buf32,

    /// Hash of the exec segment.
    // TODO ideally this is just the EL header hash, not the hash of the full payload
    pub(crate) exec_segment_hash: Buf32,

    /// State root that commits to the overall state of the rollup, commits to
    /// both the CL state and EL state.
    // TODO figure out the structure of this
    pub(crate) state_root: Buf32,

    /// Signature from this block's proposer.
    pub(crate) signature: Buf64,
}

impl L2BlockHeader {
    pub fn blockidx(&self) -> u64 {
        self.block_idx
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn parent(&self) -> &L2BlockId {
        &self.prev_block
    }

    pub fn l1_payload_hash(&self) -> &Buf32 {
        &self.l1_segment_hash
    }

    pub fn exec_payload_hash(&self) -> &Buf32 {
        &self.exec_segment_hash
    }

    pub fn state_root(&self) -> &Buf32 {
        &self.state_root
    }

    pub fn sig(&self) -> &Buf64 {
        &self.signature
    }

    /// Computes the blockid with SHA256.
    // TODO should this be poseidon?
    pub fn get_blockid(&self) -> L2BlockId {
        let buf = borsh::to_vec(self).expect("block: compute blkid");
        let h = <sha2::Sha256 as digest::Digest>::digest(&buf);
        L2BlockId::from(Buf32::from(<[u8; 32]>::from(h)))
    }
}

/// Contains the additional payloads within the L2 block.
#[derive(Clone, Debug,PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L2BlockBody {
    l1_segment: L1Segment,
    exec_segment: ExecSegment,
}

impl L2BlockBody {
    pub fn new(l1_segment: L1Segment, exec_segment: ExecSegment) -> Self {
        Self {
            l1_segment,
            exec_segment,
        }
    }

    pub fn l1_segment(&self) -> &L1Segment {
        &self.l1_segment
    }

    pub fn exec_segment(&self) -> &ExecSegment {
        &self.exec_segment
    }
}

/// Container for additional messages that we've observed from the L1, if there
/// are any.
#[derive(Clone, Debug,PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L1Segment {
    /// New headers that we've seen from L1 that we didn't see in the previous
    /// L2 block.
    new_l1_header: Vec<L1HeaderPayload>,

    /// Deposit initiation transactions.
    deposits: Vec<L1Tx>,
}

impl L1Segment {
    pub fn new(new_l1_header: Vec<L1HeaderPayload>, deposits: Vec<L1Tx>) -> Self {
        Self {
            new_l1_header,
            deposits,
        }
    }
}

/// Information relating to the EL data.
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct ExecSegment {
    /// Header of the EL data.
    el_payload: Vec<u8>,
}

impl ExecSegment {
    pub fn new(el_payload: Vec<u8>) -> Self {
        Self { el_payload }
    }

    pub fn payload(&self) -> &[u8] {
        &self.el_payload
    }
}

/// Data emitted by EL exec for a withdraw request.
#[derive(Clone, Debug)]
pub struct WithdrawData {
    /// Amount in L1 native asset.  For Bitcoin this is sats.
    amt: u64,

    /// Schnorr pubkey for the taproot output we're going to generate.
    dest_addr: Buf64,
}
