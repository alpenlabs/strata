use std::sync::Arc;

use rockbound::{OptimisticTransactionDB, SchemaBatch, SchemaDBOperationsExt};
use strata_db::{
    errors::DbError,
    traits::{BlockStatus, L2DataProvider, L2DataStore},
    DbResult,
};
use strata_state::{block::L2BlockBundle, prelude::*};

use super::schemas::{L2BlockSchema, L2BlockStatusSchema};
use crate::{l2::schemas::L2BlockHeightSchema, DbOpsConfig};

pub struct L2Db {
    db: Arc<OptimisticTransactionDB>,
    ops: DbOpsConfig,
}

impl L2Db {
    pub fn new(db: Arc<OptimisticTransactionDB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl L2DataStore for L2Db {
    fn put_block_data(&self, bundle: L2BlockBundle) -> DbResult<()> {
        let block_id = bundle.block().header().get_blockid();

        // append to previous block height data
        let block_height = bundle.block().header().blockidx();

        self.db
            .with_optimistic_txn(
                rockbound::TransactionRetry::Count(self.ops.retry_count),
                |txn| {
                    let mut block_height_data = txn
                        .get::<L2BlockHeightSchema>(&block_height)?
                        .unwrap_or(Vec::new());
                    if !block_height_data.contains(&block_id) {
                        block_height_data.push(block_id);
                    }

                    txn.put::<L2BlockSchema>(&block_id, &bundle)?;
                    txn.put::<L2BlockStatusSchema>(&block_id, &BlockStatus::Unchecked)?;
                    txn.put::<L2BlockHeightSchema>(&block_height, &block_height_data)?;

                    Ok::<_, anyhow::Error>(())
                },
            )
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn del_block_data(&self, id: L2BlockId) -> DbResult<bool> {
        let bundle = match self.get_block_data(id)? {
            Some(block) => block,
            None => return Ok(false),
        };

        // update to previous block height data
        let block_height = bundle.block().header().blockidx();
        let mut block_height_data = self.get_blocks_at_height(block_height)?;
        block_height_data.retain(|&block_id| block_id != id);

        self.db
            .with_optimistic_txn(
                rockbound::TransactionRetry::Count(self.ops.retry_count),
                |txn| {
                    let mut block_height_data = txn
                        .get::<L2BlockHeightSchema>(&block_height)?
                        .unwrap_or(Vec::new());
                    block_height_data.retain(|&block_id| block_id != id);

                    txn.delete::<L2BlockSchema>(&id)?;
                    txn.delete::<L2BlockStatusSchema>(&id)?;
                    txn.put::<L2BlockHeightSchema>(&block_height, &block_height_data)?;

                    Ok::<_, anyhow::Error>(true)
                },
            )
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn set_block_status(&self, id: L2BlockId, status: BlockStatus) -> DbResult<()> {
        if self.get_block_data(id)?.is_none() {
            return Ok(());
        }

        let mut batch = SchemaBatch::new();
        batch.put::<L2BlockStatusSchema>(&id, &status)?;
        self.db.write_schemas(batch)?;

        Ok(())
    }
}

impl L2DataProvider for L2Db {
    fn get_block_data(&self, id: L2BlockId) -> DbResult<Option<L2BlockBundle>> {
        Ok(self.db.get::<L2BlockSchema>(&id)?)
    }

    fn get_blocks_at_height(&self, idx: u64) -> DbResult<Vec<L2BlockId>> {
        Ok(self
            .db
            .get::<L2BlockHeightSchema>(&idx)?
            .unwrap_or(Vec::new()))
    }

    fn get_block_status(&self, id: L2BlockId) -> DbResult<Option<BlockStatus>> {
        Ok(self.db.get::<L2BlockStatusSchema>(&id)?)
    }
}

#[cfg(feature = "test_utils")]
#[cfg(test)]
mod tests {
    use test_utils::ArbitraryGenerator;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    fn get_mock_data() -> L2BlockBundle {
        let arb = ArbitraryGenerator::new();
        let l2_block: L2BlockBundle = arb.generate();

        l2_block
    }

    fn setup_db() -> L2Db {
        let (db, ops) = get_rocksdb_tmp_instance().unwrap();
        L2Db::new(db, ops)
    }

    #[test]
    fn set_and_get_block_data() {
        let l2_db = setup_db();

        let bundle = get_mock_data();
        let block_hash = bundle.block().header().get_blockid();
        let block_height = bundle.block().header().blockidx();

        l2_db
            .put_block_data(bundle.clone())
            .expect("failed to put block data");

        // assert block was stored
        let received_block = l2_db
            .get_block_data(block_hash)
            .expect("failed to retrieve block data")
            .unwrap();
        assert_eq!(received_block, bundle);

        // assert block status was set to `BlockStatus::Unchecked``
        let block_status = l2_db
            .get_block_status(block_hash)
            .expect("failed to retrieve block data")
            .unwrap();
        assert_eq!(block_status, BlockStatus::Unchecked);

        // assert block height data was stored
        let block_ids = l2_db
            .get_blocks_at_height(block_height)
            .expect("failed to retrieve block data");
        assert!(block_ids.contains(&block_hash))
    }

    #[test]
    fn del_and_get_block_data() {
        let l2_db = setup_db();
        let bundle = get_mock_data();
        let block_hash = bundle.block().header().get_blockid();
        let block_height = bundle.block().header().blockidx();

        // deleting non existing block should return false
        let res = l2_db
            .del_block_data(block_hash)
            .expect("failed to remove the block");
        assert!(!res);

        // deleting existing block should return true
        l2_db
            .put_block_data(bundle.clone())
            .expect("failed to put block data");
        let res = l2_db
            .del_block_data(block_hash)
            .expect("failed to remove the block");
        assert!(res);

        // assert block is deleted from the db
        let received_block = l2_db
            .get_block_data(block_hash)
            .expect("failed to retrieve block data");
        assert!(received_block.is_none());

        // assert block status is deleted from the db
        let block_status = l2_db
            .get_block_status(block_hash)
            .expect("failed to retrieve block status");
        assert!(block_status.is_none());

        // assert block height data is deleted
        let block_ids = l2_db
            .get_blocks_at_height(block_height)
            .expect("failed to retrieve block data");
        assert!(!block_ids.contains(&block_hash))
    }

    #[test]
    fn set_and_get_block_status() {
        let l2_db = setup_db();
        let bundle = get_mock_data();
        let block_hash = bundle.block().header().get_blockid();

        l2_db
            .put_block_data(bundle.clone())
            .expect("failed to put block data");

        // assert block status was set to `BlockStatus::Valid``
        l2_db
            .set_block_status(block_hash, BlockStatus::Valid)
            .expect("failed to update block status");
        let block_status = l2_db
            .get_block_status(block_hash)
            .expect("failed to retrieve block status")
            .unwrap();
        assert_eq!(block_status, BlockStatus::Valid);

        // assert block status was set to `BlockStatus::Invalid``
        l2_db
            .set_block_status(block_hash, BlockStatus::Invalid)
            .expect("failed to update block status");
        let block_status = l2_db
            .get_block_status(block_hash)
            .expect("failed to retrieve block status")
            .unwrap();
        assert_eq!(block_status, BlockStatus::Invalid);

        // assert block status was set to `BlockStatus::Unchecked``
        l2_db
            .set_block_status(block_hash, BlockStatus::Unchecked)
            .expect("failed to update block status");
        let block_status = l2_db
            .get_block_status(block_hash)
            .expect("failed to retrieve block status")
            .unwrap();
        assert_eq!(block_status, BlockStatus::Unchecked);
    }
}
