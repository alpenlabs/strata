use std::{path::Path, sync::Arc};

use tokio::sync::mpsc;

use alpen_vertex_btcio::{
    handlers::bitcoin_data_handler,
    reader::{bitcoin_data_reader, BlockData},
    rpc::BitcoinClient,
};
use alpen_vertex_db::{
    l1::{db::L1Db, utils::get_db_for_l1_store},
    traits::L1DataProvider,
};

use crate::config::RollupConfig;

pub async fn l1_reader_task(config: RollupConfig) -> anyhow::Result<()> {
    let rpc_client = BitcoinClient::new(
        config.l1_rpc_config.rpc_url,
        config.l1_rpc_config.rpc_user,
        config.l1_rpc_config.rpc_password,
        config.l1_rpc_config.network,
    );

    let db = get_db_for_l1_store(Path::new("storage-data"))?;
    let l1db = Arc::new(L1Db::new(Arc::new(db)));

    let (sender, receiver) = mpsc::channel::<BlockData>(1000); // TODO: think about the buffer size

    let start_block_number = L1DataProvider::get_chain_tip(l1db.as_ref())? + 1;

    // TODO: handle gracefully when the spawned tasks fail
    tokio::spawn(bitcoin_data_reader(rpc_client, sender, start_block_number));

    tokio::spawn(bitcoin_data_handler(l1db.clone(), receiver));
    Ok(())
}
