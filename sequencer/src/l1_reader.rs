use std::{str::FromStr, sync::Arc};

use tokio::sync::mpsc;

use alpen_vertex_btcio::{
    handlers::bitcoin_data_handler,
    reader::{bitcoin_data_reader, L1Data},
    rpc::BitcoinClient,
};
use alpen_vertex_db::traits::{Database, L1DataProvider};

use crate::args::Args;
// CommonDatabase<L1Db, StubL2Db, SyncEventDb, ConsensusStateDb>
pub async fn l1_reader_task<D>(args: Args, db: Arc<D>) -> anyhow::Result<()>
where
    D: Database,
    D::L1Prov: Sync + Send + 'static,
    D::L1Store: Sync + Send + 'static,
    D::SeStore: Sync + Send + 'static,
{
    let rpc_client = BitcoinClient::new(
        args.bitcoind_host,
        args.bitcoind_user,
        args.bitcoind_password,
        bitcoin::Network::from_str(&args.network)?,
    );

    let (sender, receiver) = mpsc::channel::<L1Data>(100); // TODO: think about the buffer size

    let l1prov = db.l1_provider().clone();
    let current_block_height = l1prov
        .get_chain_tip()?
        .unwrap_or(args.l1_start_block_height - 1);

    // TODO: handle gracefully when the spawned tasks fail
    tokio::spawn(bitcoin_data_reader(
        l1prov,
        rpc_client.clone(),
        sender,
        current_block_height,
    ));

    tokio::spawn(bitcoin_data_handler(
        db.l1_store().clone(),
        db.sync_event_store().clone(),
        receiver,
    ));
    Ok(())
}
