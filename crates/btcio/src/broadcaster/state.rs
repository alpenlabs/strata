use std::{collections::HashMap, sync::Arc};

use alpen_express_db::{
    traits::{BcastProvider, TxBroadcastDatabase},
    types::{L1TxEntry, L1TxStatus},
};

use super::error::BroadcasterResult;

pub(crate) struct BroadcasterState {
    /// Next tx idx from which we should next read the tx entries to check and process
    pub(crate) next_idx: u64,

    /// Unfinalized tx entries which the broadcaster will check for
    pub(crate) unfinalized_entries: HashMap<u64, L1TxEntry>,
}

impl BroadcasterState {
    // TODO: replace D with broadcast manager
    pub fn from_db<D: TxBroadcastDatabase + Send + Sync + 'static>(
        db: Arc<D>,
    ) -> BroadcasterResult<Self> {
        Self::from_db_start_idx(db, 0)
    }

    pub fn from_db_start_idx<D: TxBroadcastDatabase + Send + Sync + 'static>(
        db: Arc<D>,
        start_idx: u64,
    ) -> BroadcasterResult<Self> {
        let next_idx = db
            .broadcast_provider()
            .get_last_txidx()?
            .map(|x| x + 1)
            .unwrap_or(0);

        let unfinalized_entries = filter_unfinalized_from_db(db, start_idx, next_idx)?;

        Ok(Self {
            next_idx,
            unfinalized_entries,
        })
    }
}

/// Returns unfinalized and unexcluded `[L1TxEntry]`s from db starting from index `from` upto `to`
/// non-inclusive.
fn filter_unfinalized_from_db<D: TxBroadcastDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    from: u64,
    to: u64,
) -> BroadcasterResult<HashMap<u64, L1TxEntry>> {
    let mut unfinalized_entries = HashMap::new();
    for idx in from..to {
        let Some(txentry) = db.broadcast_provider().get_txentry_by_idx()? else {
            break;
        };

        match txentry.status {
            L1TxStatus::Finalized | L1TxStatus::Excluded(_) => {}
            _ => {
                unfinalized_entries.insert(idx, txentry);
            }
        }
    }
    Ok(unfinalized_entries)
}
