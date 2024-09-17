use std::ops::Deref;

use alpen_express_primitives::{evm_exec::create_evm_extra_payload, prelude::*};
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    exec_update::{self, ExecUpdate, UpdateInput, UpdateOutput},
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

impl L2BlockBundle {
    pub fn genesis(params: &Params) -> Self {
        // Create a dummy exec state that we can build the rest of the genesis block
        // around and insert into the genesis state.
        // TODO this might need to talk to the EL to do the genesus setup *properly*
        let extra_payload = create_evm_extra_payload(params.rollup.evm_genesis_block_hash);
        let geui = UpdateInput::new(0, Buf32::zero(), extra_payload);
        let genesis_update = ExecUpdate::new(
            geui.clone(),
            UpdateOutput::new_from_state(params.rollup.evm_genesis_block_state_root),
        );

        // This has to be empty since everyone should have an unambiguous view of the genesis block.
        let l1_seg = L1Segment::new_empty();

        // TODO this is a total stub, we have to fill it in with something
        let exec_seg = ExecSegment::new(genesis_update);

        let body = L2BlockBody::new(l1_seg, exec_seg);

        // TODO stub
        let exec_payload = vec![];
        let accessory = L2BlockAccessory::new(exec_payload);

        // Assemble the genesis header template, pulling in data from whatever
        // sources we need.
        // FIXME this isn't the right timestamp to start the blockchain, this should
        // definitely be pulled from the database or the rollup parameters maybe
        let genesis_ts = params.rollup().horizon_l1_height;
        let zero_blkid = L2BlockId::from(Buf32::zero());
        let genesis_sr = Buf32::zero();
        let header = L2BlockHeader::new(0, genesis_ts, zero_blkid, &body, genesis_sr);
        let signed_genesis_header = SignedL2BlockHeader::new(header, Buf64::zero());
        let block = L2Block::new(signed_genesis_header, body);
        L2BlockBundle::new(block, accessory)
    }
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::block_validation::validate_block_segments;

    #[test]
    fn test_verify_block_hashes() {
        // use arbitrary generator to get the new block
        let block: L2Block = ArbitraryGenerator::new().generate();
        assert!(validate_block_segments(&block));

        let arb_exec_segment: ExecSegment = ArbitraryGenerator::new().generate();
        let arb_l1_segment: L1Segment = ArbitraryGenerator::new().generate();
        // mutate the l2Block's body to create a new block with arbitrary exec segment
        let blk_body = L2BlockBody::new(block.body().l1_segment().clone(), arb_exec_segment);
        let arb_exec_block = L2Block::new(block.header().clone(), blk_body);
        assert!(!validate_block_segments(&arb_exec_block));

        // mutate the l2Block's body to create a new block with arbitrary l1 segment
        let blk_body = L2BlockBody::new(arb_l1_segment, block.body().exec_segment().clone());
        let arb_l1_block = L2Block::new(block.header().clone(), blk_body);
        assert!(!validate_block_segments(&arb_l1_block));
    }
}
