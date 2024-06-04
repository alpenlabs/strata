use std::{path::Path, sync::Arc};

use anyhow::anyhow;
use tracing::*;

use alpen_vertex_btcio::reader::{BlockData, BtcReader};
use alpen_vertex_db::{
    l1::{db::L1Db, utils::get_db_for_l1_store},
    traits::L1DataStore,
};
use alpen_vertex_primitives::{l1::L1BlockManifest, utils::btc_tx_data_to_l1tx};

use crate::config::RollupConfig;

pub fn handler(db: &L1Db, data: BlockData) -> anyhow::Result<()> {
    match data {
        blockdata => {
            let manifest = L1BlockManifest::from(blockdata.block().clone());
            let txns: Result<Vec<_>, _> = blockdata
                .relevant_txn_indices()
                .iter()
                .map(|&x| {
                    btc_tx_data_to_l1tx(x, blockdata.block())
                        .ok_or(anyhow!("Invalid txn in blockdata"))
                })
                .into_iter()
                .collect();

            // FIXME: blocking inside async call
            db.put_block_data(blockdata.block_num(), manifest, txns?)?;
        }
    }
    Ok(())
}

pub async fn l1_reader_task(config: RollupConfig) -> anyhow::Result<()> {
    let db = get_db_for_l1_store(Path::new("storage-data"))?;
    let l1db = L1Db::new(Arc::new(db));

    let last_block_height = l1db
        .get_latest_block_number()?
        .unwrap_or(config.l1_start_block_height - 1);

    // TODO: think about thread safety
    let handler = |data| handler(&l1db, data);
    let mut btcreader = BtcReader::new(
        &config.l1_rpc_config.zmq_endpoint,
        last_block_height,
        handler,
    )
    .await
    .expect("Could not connect to btc zmq");

    let msg = btcreader.run().await;
    warn!("{:?}", msg);
    Ok(())
}
