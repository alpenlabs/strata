//! Generic cache utility for what we're inserting into the database.

use std::{hash::Hash, num::NonZeroUsize, sync::Arc};

use strata_db::{DbError, DbResult};
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::*;

use crate::exec::DbRecv;

/// Entry for something we can put into the cache without actually knowing what it is, and so we can
/// keep the reservation to it.
type CacheSlot<T> = Arc<RwLock<SlotState<T>>>;

/// Describes a cache entry that may be occupied, reserved for pending database read, or returned an
/// error from a database read.
#[derive(Debug)]
pub enum SlotState<T> {
    /// Authentic database entry.
    Ready(T),

    /// A database fetch is happening in the background and it will be updated.
    Pending(broadcast::Receiver<T>),

    /// An unspecified error happened fetching from the database.
    Error,
}

impl<T: Clone> SlotState<T> {
    /// Tries to read a value from the slot, asynchronously.
    pub async fn get_async(&self) -> DbResult<T> {
        match self {
            Self::Ready(v) => Ok(v.clone()),
            Self::Pending(ch) => {
                // When we see this log get triggered and but feels like the corresponding fetch is
                // hanging for this read then it means that this code wasn't implemented
                // correctly.
                // TODO figure out how to test this
                trace!("waiting for database fetch to complete");
                match ch.resubscribe().recv().await {
                    Ok(v) => Ok(v),
                    Err(_e) => Err(DbError::WorkerFailedStrangely),
                }
            }
            Self::Error => Err(DbError::CacheLoadFail),
        }
    }

    /// Tries to read a value from the slot, blockingly.
    pub fn get_blocking(&self) -> DbResult<T> {
        match self {
            Self::Ready(v) => Ok(v.clone()),
            Self::Pending(ch) => {
                // When we see this log get triggered and but feels like the corresponding fetch is
                // hanging for this read then it means that this code wasn't implemented
                // correctly.
                // TODO figure out how to test this
                trace!("waiting for database fetch to complete");
                match ch.resubscribe().blocking_recv() {
                    Ok(v) => Ok(v),
                    Err(_e) => Err(DbError::WorkerFailedStrangely),
                }
            }
            Self::Error => Err(DbError::CacheLoadFail),
        }
    }
}

/// Wrapper around a LRU cache that handles cache reservations and asynchronously waiting for
/// database operations in the background without keeping a global lock on the cache.
pub struct CacheTable<K, V> {
    cache: Mutex<lru::LruCache<K, CacheSlot<V>>>,
}

impl<K: Clone + Eq + Hash, V: Clone> CacheTable<K, V> {
    /// Creates a new cache with some maximum capacity.
    ///
    /// This measures entries by *count* not their (serialized?) size, so ideally entries should
    /// consume similar amounts of memory to helps us best reason about real cache capacity.
    pub fn new(size: NonZeroUsize) -> Self {
        Self {
            cache: Mutex::new(lru::LruCache::new(size)),
        }
    }

    /// Gets the number of elements in the cache.
    // TODO replace this with an atomic we update after every op
    #[allow(dead_code)] // #FIXME: remove this.
    pub async fn get_len_async(&self) -> usize {
        let cache = self.cache.lock().await;
        cache.len()
    }

    /// Gets the number of elements in the cache.
    // TODO replace this with an atomic we update after every op
    #[allow(dead_code)] // #FIXME: remove this.
    pub fn get_len_blocking(&self) -> usize {
        let cache = self.cache.blocking_lock();
        cache.len()
    }

    /// Removes the entry for a particular cache entry.
    pub async fn purge_async(&self, k: &K) {
        let mut cache = self.cache.lock().await;
        cache.pop(k);
    }

    /// Removes the entry for a particular cache entry.
    pub fn purge_blocking(&self, k: &K) {
        let mut cache = self.cache.blocking_lock();
        cache.pop(k);
    }

    /// Inserts an entry into the table, dropping the previous value.
    #[allow(dead_code)] // #FIXME: remove this.
    pub async fn insert_async(&self, k: K, v: V) {
        let slot = Arc::new(RwLock::new(SlotState::Ready(v)));
        let mut cache = self.cache.lock().await;
        cache.put(k, slot);
    }

    /// Inserts an entry into the table, dropping the previous value.
    #[allow(dead_code)] // #FIXME: remove this.
    pub fn insert_blocking(&self, k: K, v: V) {
        let slot = Arc::new(RwLock::new(SlotState::Ready(v)));
        let mut cache = self.cache.blocking_lock();
        cache.put(k, slot);
    }

    /// Returns a clone of an entry from the cache or possibly invoking some function returning a
    /// `oneshot` channel that will return the value from the underlying database.
    ///
    /// This is meant to be used with the `_chan` functions generated by the db ops macro in the
    /// `exec` module.
    pub async fn get_or_fetch_async(&self, k: &K, fetch_fn: impl Fn() -> DbRecv<V>) -> DbResult<V> {
        // See below comment about control flow.
        let (mut slot_lock, complete_tx) = {
            let mut cache = self.cache.lock().await;
            if let Some(entry_lock) = cache.get(k) {
                let entry = entry_lock.read().await;
                return entry.get_async().await;
            }

            // Create a new cache slot and insert and lock it.
            let (complete_tx, complete_rx) = broadcast::channel(1);
            let slot = Arc::new(RwLock::new(SlotState::Pending(complete_rx)));
            cache.push(k.clone(), slot.clone());
            let lock = slot
                .try_write_owned()
                .expect("cache: lock fresh cache entry");

            (lock, complete_tx)
        };

        // Start the task and get the recv handle.
        let res_fut = fetch_fn();

        let res = match res_fut.await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                error!(?e, "failed to make database fetch");
                *slot_lock = SlotState::Error;
                self.purge_async(k).await;
                return Err(e);
            }
            Err(_) => {
                error!("database fetch aborted");
                self.purge_async(k).await;
                return Err(DbError::WorkerFailedStrangely);
            }
        };

        // Fill in the lock state and send down the complete tx.
        *slot_lock = SlotState::Ready(res.clone());
        if complete_tx.send(res.clone()).is_err() {
            warn!("failed to notify waiting cache readers");
        }

        Ok(res)
    }

    /// Returns a clone of an entry from the cache or invokes some function to load it from
    /// the underlying database.
    pub fn get_or_fetch_blocking(&self, k: &K, fetch_fn: impl Fn() -> DbResult<V>) -> DbResult<V> {
        // The flow control here is kinda weird, I don't like it.  The key here is that we want to
        // ensure the lock on the whole cache is as short-lived as possible while we check to see if
        // the entry we're looking for is there.  If it's not, then we want to insert a reservation
        // that we hold a lock to and then release the cache-level lock.
        let (mut slot_lock, complete_tx) = {
            let mut cache = self.cache.blocking_lock();
            if let Some(entry_lock) = cache.get(k) {
                let entry = entry_lock.blocking_read();
                return entry.get_blocking();
            }

            // Create a new cache slot and insert and lock it.
            let (complete_tx, complete_rx) = broadcast::channel(1);
            let slot = Arc::new(RwLock::new(SlotState::Pending(complete_rx)));
            cache.push(k.clone(), slot.clone());
            let lock = slot
                .try_write_owned()
                .expect("cache: lock fresh cache entry");

            (lock, complete_tx)
        };

        // Load the entry and insert it into the slot we've already reserved.
        let res = match fetch_fn() {
            Ok(v) => v,
            Err(e) => {
                warn!(?e, "failed to make database fetch");
                *slot_lock = SlotState::Error;
                self.purge_blocking(k);
                return Err(e);
            }
        };

        // Fill in the lock state and send down the complete tx.
        *slot_lock = SlotState::Ready(res.clone());
        if complete_tx.send(res.clone()).is_err() {
            warn!("failed to notify waiting cache readers");
        }

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use strata_db::DbError;

    use super::CacheTable;

    #[tokio::test]
    async fn test_basic_async() {
        let cache = CacheTable::<u64, u64>::new(3.try_into().unwrap());

        let res = cache
            .get_or_fetch_async(&42, || {
                let (tx, rx) = tokio::sync::oneshot::channel();
                tx.send(Ok(10)).expect("test: send init value");
                rx
            })
            .await
            .expect("test: cache gof");
        assert_eq!(res, 10);

        let res = cache
            .get_or_fetch_async(&42, || {
                let (tx, rx) = tokio::sync::oneshot::channel();
                tx.send(Err(DbError::Busy)).expect("test: send init value");
                rx
            })
            .await
            .expect("test: load gof");
        assert_eq!(res, 10);

        cache.insert_async(42, 12).await;
        let res = cache
            .get_or_fetch_async(&42, || {
                let (tx, rx) = tokio::sync::oneshot::channel();
                tx.send(Err(DbError::Busy)).expect("test: send init value");
                rx
            })
            .await
            .expect("test: load gof");
        assert_eq!(res, 12);

        let len = cache.get_len_async().await;
        assert_eq!(len, 1);
        cache.purge_async(&42).await;
        let len = cache.get_len_async().await;
        assert_eq!(len, 0);
    }

    #[test]
    fn test_basic_blocking() {
        let cache = CacheTable::<u64, u64>::new(3.try_into().unwrap());

        let res = cache
            .get_or_fetch_blocking(&42, || Ok(10))
            .expect("test: cache gof");
        assert_eq!(res, 10);

        let res = cache
            .get_or_fetch_blocking(&42, || Err(DbError::Busy))
            .expect("test: load gof");
        assert_eq!(res, 10);

        cache.insert_blocking(42, 12);
        let res = cache
            .get_or_fetch_blocking(&42, || Err(DbError::Busy))
            .expect("test: load gof");
        assert_eq!(res, 12);

        let len = cache.get_len_blocking();
        assert_eq!(len, 1);
        cache.purge_blocking(&42);
        let len = cache.get_len_blocking();
        assert_eq!(len, 0);
    }
}
