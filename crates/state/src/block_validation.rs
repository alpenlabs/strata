use alpen_express_primitives::hash;
use tracing::warn;

use crate::{block::L2Block, header::L2Header};

pub fn validate_block_segments(block: &L2Block) -> bool {
    // Check if the l1_segment_hash matches between L2Block and L2BlockHeader
    let l1seg_buf = borsh::to_vec(block.l1_segment()).expect("blockasm: enc l1 segment");
    let l1_segment_hash = hash::raw(&l1seg_buf);
    if l1_segment_hash != *block.header().l1_payload_hash() {
        warn!("computed l1_segment_hash doesn't match between L2Block and L2BlockHeader");
        return false;
    }

    // Check if the exec_segment_hash matches between L2Block and L2BlockHeader
    let eseg_buf = borsh::to_vec(block.exec_segment()).expect("blockasm: enc exec segment");
    let exec_segment_hash = hash::raw(&eseg_buf);
    if exec_segment_hash != *block.header().exec_payload_hash() {
        warn!("computed exec_segment_hash doesn't match between L2Block and L2BlockHeader");
        return false;
    }

    true
}
