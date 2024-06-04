use std::path::Path;

use tracing::*;

use alpen_vertex_btcio::reader::{BtcReader, L1Data};
use alpen_vertex_db::{
    l1::{db::L1Db, utils::get_db_for_l1_store},
    traits::L1DataStore,
};
use alpen_vertex_primitives::{l1::L1BlockManifest, utils::btc_tx_data_to_l1tx};

pub fn handler(db: &L1Db, data: L1Data) -> anyhow::Result<()> {
    match data {
        L1Data::BlockData(blockdata) => {
            let block_height = 0; // TODO: get the block height. But from where???
            let manifest = L1BlockManifest::from(blockdata.block().clone());
            let txns = blockdata
                .relevant_txn_indices()
                .iter()
                .map(|&x| btc_tx_data_to_l1tx(x, blockdata.block()))
                .collect();

            // TODO: make async call
            db.put_block_data(block_height, manifest, txns)?;
        }
    }
    Ok(())
}

pub async fn l1_reader_task() -> anyhow::Result<()> {
    let mut btcreader = BtcReader::new("tcp://127.0.0.1:29000")
        .await
        .expect("Could not connect to btc zmq");

    let db = get_db_for_l1_store(Path::new("storage-data"))?;
    let l1db = L1Db::new(db);

    let msg = btcreader.run(|data| handler(&l1db, data)).await;
    warn!("{:?}", msg);
    Ok(())
}
