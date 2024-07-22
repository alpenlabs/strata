use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::prelude::*;

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
