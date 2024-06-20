use std::io::{self, Cursor, Write};

use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::block::{L2BlockHeader, L2BlockId};

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct BlockHeaderTemplate {
    block_idx: u64,
    timestamp: u64,
    prev_block: L2BlockId,
    l1_segment_hash: Buf32,
    exec_segment_hash: Buf32,
    state_root: Buf32,
}

impl BlockHeaderTemplate {
    /// Derives the template back from the header.
    pub fn from_header(header: &L2BlockHeader) -> Self {
        Self {
            block_idx: header.blockidx(),
            timestamp: header.timestamp(),
            prev_block: *header.parent(),
            l1_segment_hash: *header.l1_payload_hash(),
            exec_segment_hash: *header.exec_payload_hash(),
            state_root: *header.state_root(),
        }
    }

    /// Compute the sighash for this block template.
    pub fn get_sighash(&self) -> Buf32 {
        // 8 + 8 + 32 + 32 + 32 + 32 = 144
        let mut buf = [0; 144];
        fill_sighash_buf(self, &mut buf).expect("blockasm: compute sighash");
        alpen_vertex_primitives::hash::raw(&buf)
    }
}

fn fill_sighash_buf(tmplt: &BlockHeaderTemplate, buf: &mut [u8]) -> Result<(), io::Error> {
    // Using a cursor here to avoid manually keeping track of indexes.  This
    // should all be optimized out to basically just memcopies.
    let mut cur = Cursor::new(&mut buf[..]);
    cur.write_all(&tmplt.block_idx.to_be_bytes())?;
    cur.write_all(&tmplt.timestamp.to_be_bytes())?;
    cur.write_all(Buf32::from(tmplt.prev_block).as_ref())?;
    cur.write_all(Buf32::from(tmplt.l1_segment_hash).as_ref())?;
    cur.write_all(Buf32::from(tmplt.exec_segment_hash).as_ref())?;
    cur.write_all(Buf32::from(tmplt.state_root).as_ref())?;

    #[cfg(test)]
    if cur.position() as usize != buf.len() {
        panic!("blockasm: did not exactly fill sighash buffer");
    }

    Ok(())
}
