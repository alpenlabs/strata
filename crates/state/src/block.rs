use std::ops::Deref;

use alpen_express_primitives::{hash, prelude::*};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use tracing::warn;

use crate::{
    exec_update,
    header::{L2BlockHeader, L2Header, SignedL2BlockHeader},
    id::L2BlockId,
    l1,
};

/// Full contents of the bare L2 block.
#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
pub struct L2Block {
    /// Header that links the block into the L2 block chain and carries the
    /// block's credential from a sequencer.
    header: SignedL2BlockHeader,

    /// Body that contains the bulk of the data.
    body: L2BlockBody,
}

impl L2Block {
    pub fn new(header: SignedL2BlockHeader, body: L2BlockBody) -> Self {
        Self { header, body }
    }

    pub fn header(&self) -> &SignedL2BlockHeader {
        &self.header
    }

    pub fn body(&self) -> &L2BlockBody {
        &self.body
    }

    pub fn l1_segment(&self) -> &L1Segment {
        &self.body.l1_segment
    }

    pub fn exec_segment(&self) -> &ExecSegment {
        &self.body.exec_segment
    }

    pub fn check_block_segments(&self) -> bool {
        // check if the l1_segment_hash matches between L2Block and L2BlockHeader
        let l1seg_buf = borsh::to_vec(self.l1_segment()).expect("blockasm: enc l1 segment");
        let l1_segment_hash = hash::raw(&l1seg_buf);
        if l1_segment_hash != *self.header().l1_payload_hash() {
            warn!("computed l1_segment_hash doesn't match between L2Block and L2BlockHeader");
            return false;
        }

        // check if the exec_segment_hash matches between L2Block and L2BlockHeader
        let eseg_buf = borsh::to_vec(self.exec_segment()).expect("blockasm: enc exec segment");
        let exec_segment_hash = hash::raw(&eseg_buf);
        if exec_segment_hash != *self.header().exec_payload_hash() {
            warn!("computed exec_segment_hash doesn't match between L2Block and L2BlockHeader");
            return false;
        }

        true
    }
}

/// Careful impl that makes the header consistent with the body.  But the prev
/// block is always 0 and the state root is random.
impl<'a> Arbitrary<'a> for L2Block {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let body = L2BlockBody::arbitrary(u)?;
        let idx = u64::arbitrary(u)?;
        let ts = u64::arbitrary(u)?;
        let prev = L2BlockId::from(Buf32::zero());
        let sr = Buf32::arbitrary(u)?;
        let header = L2BlockHeader::new(idx, ts, prev, &body, sr);
        let signed_header = SignedL2BlockHeader::new(header, Buf64::arbitrary(u)?);
        Ok(Self::new(signed_header, body))
    }
}

/// Contains the additional payloads within the L2 block.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct L1Segment {
    /// New headers that we've seen from L1 that we didn't see in the previous
    /// L2 block.
    new_payloads: Vec<l1::L1HeaderPayload>,
}

impl L1Segment {
    pub fn new(new_payloads: Vec<l1::L1HeaderPayload>) -> Self {
        Self { new_payloads }
    }

    pub fn new_empty() -> Self {
        Self::new(Vec::new())
    }

    pub fn new_payloads(&self) -> &[l1::L1HeaderPayload] {
        &self.new_payloads
    }
}

/// Information relating to how to update the execution layer.
///
/// Right now this just contains a single execution update since we only have a
/// single execution environment in our execution layer.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct ExecSegment {
    /// Update payload for the single execution environment.
    update: exec_update::ExecUpdate,
}

impl ExecSegment {
    pub fn new(update: exec_update::ExecUpdate) -> Self {
        Self { update }
    }

    /// The EE update payload.
    ///
    /// This might be replaced with a totally different scheme if we have
    /// multiple EEs.
    pub fn update(&self) -> &exec_update::ExecUpdate {
        &self.update
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshSerialize, BorshDeserialize)]
pub struct L2BlockAccessory {
    exec_payload: Vec<u8>,
}

impl L2BlockAccessory {
    pub fn new(exec_payload: Vec<u8>) -> Self {
        Self { exec_payload }
    }

    pub fn exec_payload(&self) -> &[u8] {
        &self.exec_payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct L2BlockBundle {
    block: L2Block,
    accessory: L2BlockAccessory,
}

impl L2BlockBundle {
    pub fn new(block: L2Block, accessory: L2BlockAccessory) -> Self {
        Self { block, accessory }
    }

    pub fn block(&self) -> &L2Block {
        &self.block
    }

    pub fn accessory(&self) -> &L2BlockAccessory {
        &self.accessory
    }
}

impl From<L2BlockBundle> for L2Block {
    fn from(value: L2BlockBundle) -> Self {
        value.block
    }
}

impl Deref for L2BlockBundle {
    type Target = L2Block;

    fn deref(&self) -> &Self::Target {
        &self.block
    }
}
#[cfg(test)]
mod tests {
    use alpen_test_utils::ArbitraryGenerator;

    use super::*;

    #[test]
    fn test_verify_block_hashes() {
        // use arbitrary generator to get the new block
        let block: L2Block = ArbitraryGenerator::new().generate();
        assert!(block.check_block_segments());

        let arb_exec_segment: ExecSegment = ArbitraryGenerator::new().generate();
        let arb_l1_segment: L1Segment = ArbitraryGenerator::new().generate();
        // mutate the l2Block's body to create a new block with arbitrary exec segment
        let blk_body = L2BlockBody::new(block.body().l1_segment().clone(), arb_exec_segment);
        let arb_exec_block = L2Block::new(block.header().clone(), blk_body);
        assert!(!arb_exec_block.check_block_segments());

        // mutate the l2Block's body to create a new block with arbitrary l1 segment
        let blk_body = L2BlockBody::new(arb_l1_segment, block.body().exec_segment().clone());
        let arb_l1_block = L2Block::new(block.header().clone(), blk_body);
        assert!(!arb_l1_block.check_block_segments());
    }
}
