use std::sync::Arc;

use alpen_vertex_db::traits::{L1DataStore, SyncEventStore};
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_primitives::{l1::L1BlockManifest, utils::generate_l1_tx};
use alpen_vertex_state::sync_event::SyncEvent;

use bitcoin::consensus::serialize;
use bitcoin::hashes::Hash;
use bitcoin::Block;
use tokio::sync::mpsc;
use tracing::warn;

use crate::reader::L1Data;

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
pub async fn bitcoin_data_handler<L1D, SD>(
    l1db: Arc<L1D>,
    syncdb: Arc<SD>,
    mut receiver: mpsc::Receiver<L1Data>,
) -> anyhow::Result<()>
where
    L1D: L1DataStore,
    SD: SyncEventStore,
{
    loop {
        if let Some(l1data) = receiver.recv().await {
            match l1data {
                L1Data::RevertTo(revert_block_num) => {
                    l1db.revert_to_height(revert_block_num)?;
                    // TODO: We shouldn't probably clear any sync events
                    // TODO: Write sync event, possibly send reorg event. How??
                }
                L1Data::BlockData(blockdata) => {
                    let manifest = block_to_manifest(blockdata.block().clone());
                    let l1txs: Vec<_> = blockdata
                        .relevant_txn_indices()
                        .iter()
                        .enumerate()
                        .map(|(i, _)| generate_l1_tx(i as u32, blockdata.block()))
                        .collect();
                    l1db.put_block_data(blockdata.block_num(), manifest, l1txs)?;

                    // Write to sync db
                    let blkid: Buf32 = blockdata.block().block_hash().into();
                    let idx = syncdb.write_sync_event(SyncEvent::L1Block(
                        blockdata.block_num(),
                        blkid.into(),
                    ))?;
                    // TODO: Should send idx to any receivers?
                }
            }
        } else {
            warn!("Bitcoin reader sent None blockdata");
        }
    }
}
