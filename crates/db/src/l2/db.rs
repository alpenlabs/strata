use std::sync::Arc;
use alpen_vertex_state::block::{L2Block, L2BlockId};
use rockbound::{schema::KeyEncoder, SchemaBatch, DB};

use crate::{traits::{BlockStatus, L2DataProvider, L2DataStore}, DbResult};


pub struct L2Db {
    db: Arc<DB>,

}

impl L2Db{
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

impl L2DataStore for L2Db {
    fn put_block_data(&self, block: L2Block) -> DbResult<()> {
        todo!()
    }

    fn del_block_data(&self, id: L2BlockId) -> DbResult<bool> {
        todo!()
    }

    fn set_block_status(&self, id: L2BlockId, status: BlockStatus) -> DbResult<()> {
        todo!()
    }
}

impl L2DataProvider for L2Db{
    fn get_block_data(&self, id: L2BlockId) -> DbResult<Option<L2Block>> {
        todo!()
    }

    fn get_blocks_at_height(&self, idx: u64) -> DbResult<Vec<L2BlockId>> {
        todo!()
    }

    fn get_block_status(&self, id: L2BlockId) -> DbResult<Option<BlockStatus>> {
        todo!()
    }
}


#[cfg(test)]
mod tests {
    use std::path::Path;
    use rockbound::schema::ColumnFamilyName;
    use rocksdb::Options;
    use tempfile::TempDir;
    use crate::STORE_COLUMN_FAMILIES;

    use super::*;

    const DB_NAME: &str = "l2_db";

    fn get_new_db(path:&Path)->anyhow::Result<Arc<DB>>{
        let mut db_opts = Options::default();
        db_opts.create_missing_column_families(true);
        db_opts.create_if_missing(true);

        DB::open(
            path,
            DB_NAME,
            STORE_COLUMN_FAMILIES
                .iter()
                .cloned()
                .collect::<Vec<ColumnFamilyName>>(),
            &db_opts,
        )
        .map(Arc::new)
    }

    fn setup_db() -> L2Db {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let db = get_new_db(&temp_dir.into_path()).unwrap();
        L2Db::new(db)
    }


    #[test]
    fn put_block_data(){
        let db = setup_db(); 
    }

    

}