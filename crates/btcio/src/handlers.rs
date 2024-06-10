use std::sync::Arc;

use alpen_vertex_db::traits::{L1DataProvider, L1DataStore};
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_primitives::{l1::L1BlockManifest, utils::generate_l1_tx};

use bitcoin::consensus::serialize;
use bitcoin::hashes::Hash;
use bitcoin::Block;
use tokio::sync::mpsc;
use tracing::warn;

use crate::reader::BlockData;
use crate::reorg::detect_reorg;
use crate::rpc::BitcoinClient;

pub fn block_to_manifest(block: Block) -> L1BlockManifest {
    let blockid = Buf32(block.block_hash().to_raw_hash().to_byte_array().into());
    let root = block
        .witness_root()
        .map(|x| x.to_byte_array())
        .unwrap_or_default();
    let header = serialize(&block.header);

    L1BlockManifest::new(blockid, header, Buf32(root.into()))
}

/// This consumes data passed through channel by bitcoin_data_reader() task
pub async fn bitcoin_data_handler<D>(
    l1db: Arc<D>,
    mut receiver: mpsc::Receiver<BlockData>,
    rpc_client: BitcoinClient,
) -> anyhow::Result<()>
where
    D: L1DataProvider + L1DataStore,
{
    loop {
        if let Some(blockdata) = receiver.recv().await {
            if let Some(reorg_block_num) = detect_reorg(&l1db, &blockdata, &rpc_client).await? {
                l1db.revert_to_height(reorg_block_num)?;
                continue;
            }
            let manifest = block_to_manifest(blockdata.block().clone());
            let l1txs: Vec<_> = blockdata
                .relevant_txn_indices()
                .iter()
                .enumerate()
                .map(|(i, _)| generate_l1_tx(i as u32, blockdata.block()))
                .collect();
            l1db.put_block_data(blockdata.block_num(), manifest, l1txs)?;
        } else {
            warn!("Bitcoin reader sent None blockdata");
        }
    }
}
