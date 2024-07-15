use std::collections::*;

use parking_lot::Mutex;

use alpen_vertex_state::prelude::*;

use crate::errors::*;
use crate::traits::*;

/// Dummy implementation that isn't really compliant with the spec, but we don't
/// care because we just want to get something running. :sunglasses:.
pub struct StubL2Db {
    blocks: Mutex<HashMap<L2BlockId, L2Block>>,
    statuses: Mutex<HashMap<L2BlockId, BlockStatus>>,
    heights: Mutex<HashMap<u64, Vec<L2BlockId>>>,
}

impl Default for StubL2Db {
    fn default() -> Self {
        Self::new()
    }
}

impl StubL2Db {
    pub fn new() -> Self {
        Self {
            blocks: Mutex::new(HashMap::new()),
            statuses: Mutex::new(HashMap::new()),
            heights: Mutex::new(HashMap::new()),
        }
    }
}

impl L2DataStore for StubL2Db {
    fn put_block_data(&self, block: L2Block) -> DbResult<()> {
        let blkid = block.header().get_blockid();
        let idx = block.header().blockidx();

        {
            let mut tbl = self.blocks.lock();
            tbl.insert(blkid, block);
        }

        {
            let mut tbl = self.heights.lock();
            tbl.entry(idx).or_insert_with(Vec::new).push(blkid);
        }

        Ok(())
    }

    fn del_block_data(&self, id: L2BlockId) -> DbResult<bool> {
        let mut tbl = self.blocks.lock();
        Ok(tbl.remove(&id).is_some())
    }

    fn set_block_status(&self, id: L2BlockId, status: BlockStatus) -> DbResult<()> {
        let mut tbl = self.statuses.lock();
        tbl.insert(id, status);
        Ok(())
    }
}

impl L2DataProvider for StubL2Db {
    fn get_block_data(&self, id: L2BlockId) -> DbResult<Option<L2Block>> {
        let tbl = self.blocks.lock();
        Ok(tbl.get(&id).cloned())
    }

    fn get_blocks_at_height(&self, idx: u64) -> DbResult<Vec<L2BlockId>> {
        let tbl = self.heights.lock();
        Ok(tbl.get(&idx).cloned().unwrap_or_default())
    }

    fn get_block_status(&self, id: L2BlockId) -> DbResult<Option<BlockStatus>> {
        let tbl = self.statuses.lock();
        Ok(tbl.get(&id).cloned())
    }
}
