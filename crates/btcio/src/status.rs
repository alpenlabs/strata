use bitcoin::Txid;
use strata_status::StatusChannel;

#[derive(Debug, Clone)]
pub enum L1StatusUpdate {
    CurHeight(u64),
    LastUpdate(u64),
    RpcConnected(bool),
    RpcError(String),
    CurTip(String),
    LastPublishedTxid(Txid),
    IncrementInscriptionCount,
}

pub async fn apply_status_updates(st_updates: &[L1StatusUpdate], st_chan: &StatusChannel) {
    let mut l1_status = st_chan.l1_status();
    for event in st_updates {
        match event {
            L1StatusUpdate::CurHeight(height) => l1_status.cur_height = *height,
            L1StatusUpdate::LastUpdate(epoch_time) => l1_status.last_update = *epoch_time,
            L1StatusUpdate::RpcConnected(connected) => l1_status.bitcoin_rpc_connected = *connected,
            L1StatusUpdate::RpcError(err_string) => {
                l1_status.last_rpc_error = Some(err_string.clone())
            }
            L1StatusUpdate::CurTip(tip) => l1_status.cur_tip_blkid = tip.clone(),
            L1StatusUpdate::LastPublishedTxid(txid) => {
                l1_status.last_published_txid = Some(Into::into(*txid))
            }
            L1StatusUpdate::IncrementInscriptionCount => l1_status.published_inscription_count += 1,
        }
    }

    st_chan.update_l1_status(l1_status);
}
