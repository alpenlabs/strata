use std::sync::Arc;

use bitcoin::consensus::serialize;
use bitcoin::hashes::Hash;
use bitcoin::Block;
use tokio::sync::mpsc;
use tracing::*;

use alpen_vertex_db::traits::{L1DataStore, SyncEventStore};
use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_primitives::{l1::L1BlockManifest, utils::generate_l1_tx};
use alpen_vertex_state::sync_event::SyncEvent;

use super::config::ReaderConfig;
use crate::reader::L1Event;

/// Consumes L1 events and reflects them in the database.
pub fn bitcoin_data_handler_task<L1D, SD>(
    l1db: Arc<L1D>,
    syncdb: Arc<SD>,
    mut event_rx: mpsc::Receiver<L1Event>,
    _config: Arc<ReaderConfig>,
) -> anyhow::Result<()>
where
    L1D: L1DataStore + Sync + Send + 'static,
    SD: SyncEventStore + Sync + Send + 'static,
{
    loop {
        let Some(event) = event_rx.blocking_recv() else {
            break;
        };

        match event {
            L1Event::RevertTo(revert_blk_num) => {
                // L1 reorgs will be handled in L2 STF, we just have to reflect
                // what the client is telling us in the database.
                l1db.revert_to_height(revert_blk_num)?;
                debug!(%revert_blk_num, "wrote revert");
            }

            L1Event::BlockData(blockdata) => {
                let l1blkid = blockdata.block().block_hash();

                let manifest = generate_block_manifest(blockdata.block());
                let l1txs: Vec<_> = blockdata
                    .interesting_tx_idxs()
                    .iter()
                    .enumerate()
                    .map(|(i, _)| generate_l1_tx(i as u32, blockdata.block()))
                    .collect();
                let num_txs = l1txs.len();

                l1db.put_block_data(blockdata.block_num(), manifest, l1txs)?;
                info!(%l1blkid, txs = %num_txs, "wrote L1 block manifest");

                // Write to sync event db.
                let blkid: Buf32 = blockdata.block().block_hash().into();
                let _idx = syncdb
                    .write_sync_event(SyncEvent::L1Block(blockdata.block_num(), blkid.into()))?;
                // TODO Send idx to any receivers that might be listening to this
            }
        }
    }

    info!("L1 reader task exiting");
    Ok(())
}

/// Given a block, generates a manifest of the parts we care about that we can
/// store in the database.
fn generate_block_manifest(block: &Block) -> L1BlockManifest {
    let blockid = Buf32::from(block.block_hash().to_raw_hash().to_byte_array());
    let root = block
        .witness_root()
        .map(|x| x.to_byte_array())
        .unwrap_or_default();
    let header = serialize(&block.header);

    L1BlockManifest::new(blockid, header, Buf32::from(root))
}
