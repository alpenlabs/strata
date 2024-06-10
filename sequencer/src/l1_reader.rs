use std::{str::FromStr, sync::Arc};

use tokio::sync::mpsc;

use alpen_vertex_btcio::{
    handlers::bitcoin_data_handler,
    reader::{bitcoin_data_reader, BlockData},
    rpc::BitcoinClient,
};
use alpen_vertex_db::{
    database::CommonDatabase, l1::db::L1Db, stubs::l2::StubL2Db, traits::Database,
    ConsensusStateDb, SyncEventDb,
};

use crate::args::Args;

pub async fn l1_reader_task(
    args: Args,
    db: Arc<CommonDatabase<L1Db, StubL2Db, SyncEventDb, ConsensusStateDb>>,
) -> anyhow::Result<()> {
    let rpc_client = BitcoinClient::new(
        args.bitcoin_rpc_url,
        args.bitcoin_rpc_user,
        args.bitcoin_rpc_password,
        bitcoin::Network::from_str(&args.bitcoin_rpc_network)?,
    );

    let (sender, receiver) = mpsc::channel::<BlockData>(100); // TODO: think about the buffer size

    // TODO: handle gracefully when the spawned tasks fail
    tokio::spawn(bitcoin_data_reader(
        db.l1_store().clone(),
        rpc_client.clone(),
        sender,
        args.l1_start_block_height,
    ));

    tokio::spawn(bitcoin_data_handler(
        db.l1_store().clone(),
        db.sync_event_store().clone(),
        receiver,
        rpc_client,
    ));
    Ok(())
}
