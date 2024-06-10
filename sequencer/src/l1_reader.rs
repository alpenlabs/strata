use std::{str::FromStr, sync::Arc};

use tokio::sync::mpsc;

use alpen_vertex_btcio::{
    handlers::bitcoin_data_handler,
    reader::{bitcoin_data_reader, BlockData},
    rpc::BitcoinClient,
};
use alpen_vertex_db::l1::db::L1Db;

use crate::args::Args;

pub async fn l1_reader_task(args: Args, rbdb: Arc<rockbound::DB>) -> anyhow::Result<()> {
    let rpc_client = BitcoinClient::new(
        args.bitcoin_rpc_url,
        args.bitcoin_rpc_user,
        args.bitcoin_rpc_password,
        bitcoin::Network::from_str(&args.bitcoin_rpc_network)?,
    );

    let l1db = Arc::new(L1Db::new(rbdb));

    let (sender, receiver) = mpsc::channel::<BlockData>(1000); // TODO: think about the buffer size

    // TODO: handle gracefully when the spawned tasks fail
    tokio::spawn(bitcoin_data_reader(
        l1db.clone(),
        rpc_client.clone(),
        sender,
        args.l1_start_block_height,
    ));

    tokio::spawn(bitcoin_data_handler(l1db, receiver, rpc_client));
    Ok(())
}
