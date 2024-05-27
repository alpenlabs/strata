use rockbound::{Schema, SchemaBatch, DB};
use rocksdb::Options;
use std::path::Path;

use alpen_vertex_state::sync_event::SyncEvent;

use crate::{DbResult, traits::SyncEventStore};
use crate::traits::SyncEventProvider;

use super::schemas::{SyncEventSchema, SyncEventWithTimestamp};

const DB_NAME: &str = "se_db";

pub struct SeDb {
    db: DB
}

fn get_db_opts() -> Options {
    // TODO: add other options as appropriate.
    let mut db_opts = Options::default();
    db_opts.create_missing_column_families(true);
    db_opts.create_if_missing(true);
    db_opts
}

impl SeDb {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let db_opts = get_db_opts();
        let column_families = vec![
            SyncEventSchema::COLUMN_FAMILY_NAME
        ];
        let store = Self {
            db: DB::open(path, DB_NAME, column_families, &db_opts)?
        };
        Ok(store)
    }

    fn last_idx(&self) -> DbResult<Option<u64>> {
        let mut iterator = self.db.iter::<SyncEventSchema>()?;
        iterator.seek_to_last();
        match iterator.rev().next() {
            Some(res) => {
                let (tip, _) = res?.into_tuple();
                Ok(Some(tip))
            },
            None => Ok(None)
        }
    }
}

impl SyncEventStore for SeDb {
    fn write_sync_event(&self, ev: SyncEvent) -> DbResult<u64> {
        let last_id = self.get_last_idx()?.unwrap_or(0);
        let id = last_id + 1;
        let event = SyncEventWithTimestamp::new(ev);
        self.db.put::<SyncEventSchema>(&id, &event)?;
        Ok(id)
    }

    fn clear_sync_event(&self, start_idx: u64, end_idx: u64) -> DbResult<()> {
        let iterator = self.db.iter::<SyncEventSchema>()?;

        // TODO: determine if the expectation behaviour for this is to clear early events or clear late events
        // The implementation is based for early events
        let mut batch = SchemaBatch::new();

        for res in iterator {
            let (id, _) = res?.into_tuple();
            if id > end_idx {
                break;
            }

            if id >= start_idx {
                batch.delete::<SyncEventSchema>(&id)?;
            }
        }
        self.db.write_schemas(batch)?;
        Ok(())
    }

}

impl SyncEventProvider for SeDb {
    fn get_last_idx(&self) -> DbResult<Option<u64>> {
        self.last_idx()
    }

    fn get_sync_event(&self, idx: u64) -> DbResult<Option<SyncEvent>> {
        let event = self.db.get::<SyncEventSchema>(&idx)?;
        match event {
            Some(ev) => Ok(Some(ev.event())),
            None => Ok(None)
        }
    }

    fn get_event_timestamp(&self, idx: u64) -> DbResult<Option<u64>> {
        let event = self.db.get::<SyncEventSchema>(&idx)?;
        match event {
            Some(ev) => Ok(Some(ev.timestamp())),
            None => Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use arbitrary::{Arbitrary, Unstructured};
    use tempfile::TempDir;
    use SeDb;

    fn generate_arbitrary<'a, T: Arbitrary<'a> + Clone>() -> T {
        let mut u = Unstructured::new(&[1, 2, 3]);
        T::arbitrary(&mut u).expect("failed to generate arbitrary instance")
    }

    fn setup_db() -> SeDb {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        SeDb::new(temp_dir.path()).expect("failed to create L1Db")
    }

    fn insert_event(db: &SeDb) -> SyncEvent {
        let ev: SyncEvent = generate_arbitrary();
        let res = db.write_sync_event(ev.clone());
        assert!(res.is_ok());
        ev
    }

    #[test]
    fn test_initialization() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let db = SeDb::new(temp_dir.path());
        assert!(db.is_ok());
    }

    #[test]
    fn test_get_sync_event() {
        let db = setup_db();

        let ev1 = db.get_sync_event(1).unwrap();
        assert!(ev1.is_none());

        let ev = insert_event(&db);

        let ev1 = db.get_sync_event(1).unwrap();
        assert!(ev1.is_some());

        assert_eq!(ev1.unwrap(), ev);
    }

    #[test]
    fn test_get_last_idx_1() {
        let db = setup_db();

        let idx = db.get_last_idx().unwrap().unwrap_or(0);
        assert_eq!(idx, 0);

        let n = 5;
        for i in 1..=n {
            let _ = insert_event(&db);
            let idx = db.get_last_idx().unwrap().unwrap_or(0);
            assert_eq!(idx, i);
        }
    }

    #[test]
    fn test_get_timestamp() {
        let db = setup_db();
        let mut timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        let n = 5;
        for i in 1..=n {
            let _ = insert_event(&db);
            let ts = db.get_event_timestamp(i).unwrap().unwrap();
            assert!(ts >= timestamp);
            timestamp = ts;
        }
    }

    #[test]
    fn test_clear_sync_event() {
        let db = setup_db();
        let n = 5;
        for _ in 1..=n {
            let _ = insert_event(&db);
        }
        // Delete events 2..=4
        let res = db.clear_sync_event(2,4);
        assert!(res.is_ok());

        let ev1 = db.get_sync_event(1).unwrap();
        let ev2 = db.get_sync_event(2).unwrap();
        let ev3 = db.get_sync_event(3).unwrap();
        let ev4 = db.get_sync_event(4).unwrap();
        let ev5 = db.get_sync_event(5).unwrap();

        assert!(ev1.is_some());
        assert!(ev2.is_none());
        assert!(ev3.is_none());
        assert!(ev4.is_none());
        assert!(ev5.is_some());
    }

    #[test]
    fn test_clear_sync_event_2() {
        let db = setup_db();
        let n = 5;
        for _ in 1..=n {
            let _ = insert_event(&db);
        }
        let res = db.clear_sync_event(6, 7);
        assert!(res.is_ok());
    }


    #[test]
    fn test_get_last_idx_2() {
        let db = setup_db();
        let n = 5;
        for _ in 1..=n {
            let _ = insert_event(&db);
        }
        let res = db.clear_sync_event(2,3);
        assert!(res.is_ok());

        let new_idx = db.get_last_idx().unwrap().unwrap();
        assert_eq!(new_idx, 5);
    }

    #[test]
    fn test_get_last_idx_3() {
        let db = setup_db();
        let n = 5;
        for _ in 1..=n {
            let _ = insert_event(&db);
        }
        let res = db.clear_sync_event(3,5);
        assert!(res.is_ok());

        let new_idx = db.get_last_idx().unwrap().unwrap();
        assert_eq!(new_idx, 2);
    }

}