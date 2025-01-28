use std::{
    fmt::{Debug, Display},
    fs,
    path::PathBuf,
};

use anyhow::{bail, Context};
use bincode::Options;
use rockbound::{rocksdb, SchemaDBOperations, SchemaDBOperationsExt};

pub fn db_upgrade(datadir: PathBuf, commit: bool) -> anyhow::Result<()> {
    let db = open_rocksdb_database(datadir)?;

    let cf = db.get_cf_handle(CHECKPOINT_SCHEMA)?;

    let mut existing_keys = Vec::new();
    let mut new_entries = Vec::new();

    let iter = db.db().iterator_cf(cf, rocksdb::IteratorMode::Start);

    let bincode_options = bincode::options().with_fixint_encoding().with_big_endian();

    for entry in iter {
        let (k, v) = match entry {
            Ok(entry) => entry,
            Err(err) => bail!("failed to read entry: {}", err),
        };
        let key: u64 = borsh::from_slice(&k).expect("decode borsh serialized key");
        existing_keys.push((key, k));
        let corrected_encoding_key = bincode_options
            .serialize(&key)
            .expect("encode key with bincode");

        new_entries.push((key, corrected_encoding_key, v));
    }

    // sort by key
    new_entries.sort_by_key(|(i, _, _)| *i);

    println!("existing: {}", existing_keys.len());
    display_checkpoints(existing_keys.iter().map(|(i, k)| (i, k)));
    println!("new_entries: {}", new_entries.len());
    display_checkpoints(new_entries.iter().map(|(i, k, _)| (i, k)));

    // ensure all keys are present
    let all_keys_sequential = new_entries
        .iter()
        .map(|(i, _, _)| *i)
        .eq(0..new_entries.len() as u64);

    if !all_keys_sequential {
        bail!("missing keys");
    }

    if !commit {
        return Ok(());
    }

    let mut batch = rocksdb::WriteBatch::default();

    for (_, key) in existing_keys {
        batch.delete_cf(cf, key);
    }

    for (_, key, value) in new_entries {
        batch.put_cf(cf, key, value);
    }

    db.db().write(batch)?;

    Ok(())
}

// hardcoding to specific schema at releases/0.1.0
const CHECKPOINT_SCHEMA: &str = "BatchCheckpointSchema";
const STORE_COLUMN_FAMILIES: &[&str] = &[
    "SequenceSchema",
    "ChainStateSchema",
    "ClientUpdateOutputSchema",
    "ClientStateSchema",
    "L1BlockSchema",
    "MmrSchema",
    "SyncEventSchema",
    "TxnSchema",
    "L2BlockSchema",
    "L2BlockStatusSchema",
    "L2BlockHeightSchema",
    "WriteBatchSchema",
    "SeqBlobIdSchema",
    "SeqBlobSchema",
    "BcastL1TxIdSchema",
    "BcastL1TxSchema",
    "BridgeMsgIdSchema",
    "ScopeMsgIdSchema",
    "BridgeTxStateTxidSchema",
    "BridgeTxStateSchema",
    "BridgeDutyTxidSchema",
    "BridgeDutyStatusSchema",
    "BridgeDutyCheckpointSchema",
    CHECKPOINT_SCHEMA,
];

fn open_rocksdb_database(datadir: PathBuf) -> anyhow::Result<rockbound::DB> {
    let database_dir = datadir;

    if !database_dir.exists() {
        fs::create_dir_all(&database_dir)?;
    }

    let dbname = strata_rocksdb::ROCKSDB_NAME;
    let cfs = STORE_COLUMN_FAMILIES;

    dbg!(&cfs);

    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(false);
    opts.create_missing_column_families(false);

    let rbdb = rockbound::DB::open(
        &database_dir,
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )
    .context("opening database")?;

    Ok(rbdb)
}

fn display_checkpoints<T: Display, U: Debug>(items: impl Iterator<Item = (T, U)>) {
    for (key, bytes) in items {
        println!("{}: {:?}", key, bytes);
    }
}
