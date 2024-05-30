use std::path::Path;

use alpen_vertex_db::{
    l1::{db::L1Db, utils::get_db_for_l1_store},
    traits::{L1BlockManifest, L1DataStore},
};
use alpen_vertex_primitives::l1::L1Tx;
use tracing::*;

use alpen_vertex_btcio::{BtcIO, L1Data};

pub fn handler(db: &L1Db, data: L1Data) -> anyhow::Result<()> {
    match data {
        L1Data::BlockData(blockdata) => {
            let block_height = 0; // TODO: get the block height. But from where???
            let manifest = L1BlockManifest::from(blockdata.block().clone());
            let txns = blockdata
                .relevant_txns()
                .iter()
                .map(|&x| L1Tx::from((x, blockdata.block())))
                .collect(); // TODO: create this appropriately

            // TODO: insert appropriate values
            db.put_block_data(block_height, manifest, txns)?;
        }
    }
    Ok(())
}

pub async fn l1_reader_task() -> anyhow::Result<()> {
    let mut btcio = BtcIO::new("tcp://127.0.0.1:29000")
        .await
        .expect("Could not connect to btc zmq");

    let db = get_db_for_l1_store(Path::new("storage-data"))?;
    let l1db = L1Db::new(db);

    let msg = btcio.run(|data| handler(&l1db, data)).await;
    warn!("{:?}", msg);
    Ok(())
}
