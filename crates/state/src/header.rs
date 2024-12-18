use std::io::{self, Cursor, Write};

use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::{
    buf::{Buf32, Buf64},
    hash,
};

use crate::{block::L2BlockBody, id::L2BlockId};

pub trait L2Header {
    fn blockidx(&self) -> u64;
    fn timestamp(&self) -> u64;
    fn parent(&self) -> &L2BlockId;
    fn l1_payload_hash(&self) -> &Buf32;
    fn exec_payload_hash(&self) -> &Buf32;
    fn state_root(&self) -> &Buf32;
    fn get_blockid(&self) -> L2BlockId;
}

/// Block header that forms the chain we use to reach consensus.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Serialize, Deserialize,
)]
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
}

impl L2BlockHeader {
    /// Creates a new L2BlockHeader
    pub fn new(
        block_idx: u64,
        timestamp: u64,
        prev_block: L2BlockId,
        body: &L2BlockBody,
        state_root: Buf32,
    ) -> Self {
        let l1seg_buf = borsh::to_vec(body.l1_segment()).expect("blockasm: enc l1 segment");
        let l1_segment_hash = hash::raw(&l1seg_buf);
        let eseg_buf = borsh::to_vec(body.exec_segment()).expect("blockasm: enc exec segment");
        let exec_segment_hash = hash::raw(&eseg_buf);
        L2BlockHeader {
            block_idx,
            timestamp,
            prev_block,
            l1_segment_hash,
            exec_segment_hash,
            state_root,
        }
    }
    /// Compute the sighash for this block header.
    pub fn get_sighash(&self) -> Buf32 {
        // 8 + 8 + 32 + 32 + 32 + 32 = 144
        let mut buf = [0; 144];
        fill_sighash_buf(self, &mut buf).expect("blockasm: compute sighash");
        strata_primitives::hash::raw(&buf)
    }
}

impl From<SignedL2BlockHeader> for L2BlockHeader {
    fn from(signed: SignedL2BlockHeader) -> Self {
        signed.header
    }
}

impl L2Header for L2BlockHeader {
    fn blockidx(&self) -> u64 {
        self.block_idx
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn parent(&self) -> &L2BlockId {
        &self.prev_block
    }

    fn l1_payload_hash(&self) -> &Buf32 {
        &self.l1_segment_hash
    }

    fn exec_payload_hash(&self) -> &Buf32 {
        &self.exec_segment_hash
    }

    fn state_root(&self) -> &Buf32 {
        &self.state_root
    }

    fn get_blockid(&self) -> L2BlockId {
        self.get_sighash().into()
    }
}

fn fill_sighash_buf(tmplt: &L2BlockHeader, buf: &mut [u8]) -> Result<(), io::Error> {
    // Using a cursor here to avoid manually keeping track of indexes.  This
    // should all be optimized out to basically just memcopies.
    let mut cur = Cursor::new(&mut buf[..]);
    cur.write_all(&tmplt.block_idx.to_be_bytes())?;
    cur.write_all(&tmplt.timestamp.to_be_bytes())?;
    cur.write_all(Buf32::from(tmplt.prev_block).as_ref())?;
    cur.write_all(tmplt.l1_segment_hash.as_ref())?;
    cur.write_all(tmplt.exec_segment_hash.as_ref())?;
    cur.write_all(tmplt.state_root.as_ref())?;

    #[cfg(test)]
    if cur.position() as usize != buf.len() {
        panic!("blockasm: did not exactly fill sighash buffer");
    }

    Ok(())
}

/// Block header that forms the chain we use to reach consensus.
#[derive(
    Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize, Serialize, Deserialize,
)]
pub struct SignedL2BlockHeader {
    pub(crate) header: L2BlockHeader,

    /// Signature from this block's proposer.
    pub(crate) signature: Buf64,
}

impl SignedL2BlockHeader {
    pub fn new(header: L2BlockHeader, sig: Buf64) -> Self {
        SignedL2BlockHeader {
            header,
            signature: sig,
        }
    }

    pub fn sig(&self) -> &Buf64 {
        &self.signature
    }

    pub fn header(&self) -> &L2BlockHeader {
        &self.header
    }
}

impl L2Header for SignedL2BlockHeader {
    fn blockidx(&self) -> u64 {
        self.header.blockidx()
    }

    fn timestamp(&self) -> u64 {
        self.header.timestamp()
    }

    fn parent(&self) -> &L2BlockId {
        self.header.parent()
    }

    fn l1_payload_hash(&self) -> &Buf32 {
        self.header.l1_payload_hash()
    }

    fn exec_payload_hash(&self) -> &Buf32 {
        self.header.exec_payload_hash()
    }

    fn state_root(&self) -> &Buf32 {
        &self.header.state_root
    }

    fn get_blockid(&self) -> L2BlockId {
        self.header.get_blockid()
    }
}
