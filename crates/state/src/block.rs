use std::ops::Deref;

use alpen_express_primitives::prelude::*;
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    exec_update,
    header::{L2BlockHeader, SignedL2BlockHeader},
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
