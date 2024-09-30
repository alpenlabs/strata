use std::{
    cell::RefCell,
    io,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    rc::Rc,
    sync::OnceLock,
};

use bdk_esplora::{
    esplora_client::{self, AsyncClient},
    EsploraAsyncExt,
};
use bdk_wallet::{
    bitcoin::{FeeRate, Network},
    rusqlite::{self, Connection},
    ChangeSet, PersistedWallet, WalletPersister,
};
use console::{style, Term};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::seed::Seed;

/// Retrieves an estimated fee rate to settle a transaction in `target` blocks
pub async fn get_fee_rate(
    target: u16,
    esplora_client: &AsyncClient,
) -> Result<Option<FeeRate>, esplora_client::Error> {
    Ok(esplora_client
        .get_fee_estimates()
        .await
        .map(|frs| frs.get(&target).cloned())?
        .map(|fr| FeeRate::from_sat_per_vb(fr as u64))
        .flatten())
}

pub fn log_fee_rate(term: &Term, fr: &FeeRate) {
    let _ = term.write_line(&format!(
        "Using {} as feerate",
        style(format!("~{} sat/vb", fr.to_sat_per_vb_ceil())).green(),
    ));
}

#[derive(Clone)]
pub struct EsploraClient(AsyncClient);

impl DerefMut for EsploraClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for EsploraClient {
    type Target = AsyncClient;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl EsploraClient {
    pub fn new(esplora_url: &str) -> Result<Self, esplora_client::Error> {
        Ok(Self(
            esplora_client::Builder::new(esplora_url).build_async()?,
        ))
    }
}

/// A wrapper around BDK's wallet with some custom logic
#[derive(Debug)]
pub struct SignetWallet(PersistedWallet<Persister>);

impl SignetWallet {
    fn db_path(wallet: &str, data_dir: &Path) -> PathBuf {
        data_dir.join(wallet).with_extension("sqlite")
    }

    pub fn persister(data_dir: &Path) -> Result<Connection, rusqlite::Error> {
        Connection::open(Self::db_path("default", data_dir))
    }

    pub fn new(seed: &Seed, network: Network) -> io::Result<Self> {
        let (load, create) = seed.signet_wallet().split();
        Ok(Self(
            load.check_network(network)
                .load_wallet(&mut Persister)
                .expect("should be able to load wallet")
                .unwrap_or_else(|| {
                    create
                        .network(network)
                        .create_wallet(&mut Persister)
                        .expect("wallet creation to succeed")
                }),
        ))
    }

    pub async fn sync(
        &mut self,
        esplora_client: &AsyncClient,
    ) -> Result<(), Box<esplora_client::Error>> {
        let term = Term::stdout();
        let _ = term.write_line("Syncing wallet...");
        let sty = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-");

        let bar = MultiProgress::new();

        let ops = bar.add(ProgressBar::new(1));
        ops.set_style(sty.clone());
        ops.set_message("outpoints");
        let ops2 = ops.clone();

        let spks = bar.add(ProgressBar::new(1));
        spks.set_style(sty.clone());
        spks.set_message("script public keys");
        let spks2 = spks.clone();

        let txids = bar.add(ProgressBar::new(1));
        txids.set_style(sty.clone());
        txids.set_message("transactions");
        let txids2 = txids.clone();
        let req = self
            .start_sync_with_revealed_spks()
            .inspect(move |item, progress| {
                let _ = bar.println(format!("{item}"));
                ops.set_length(progress.total_outpoints() as u64);
                ops.set_position(progress.outpoints_consumed as u64);
                spks.set_length(progress.total_spks() as u64);
                spks.set_position(progress.spks_consumed as u64);
                txids.set_length(progress.total_txids() as u64);
                txids.set_length(progress.txids_consumed as u64);
            })
            .build();

        let update = esplora_client.sync(req, 3).await?;
        ops2.finish();
        spks2.finish();
        txids2.finish();
        self.apply_update(update)
            .expect("should be able to connect to db");
        self.persist().expect("persist should work");
        let _ = term.write_line("Wallet synced");
        Ok(())
    }

    pub fn persist(&mut self) -> Result<bool, rusqlite::Error> {
        self.0.persist(&mut Persister)
    }
}

impl Deref for SignetWallet {
    type Target = PersistedWallet<Persister>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SignetWallet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Wrapper around the built-in rusqlite db that allows [`PersistedWallet`] to be
/// shared across multiple threads by lazily initializing per core connections
/// to the sqlite db and keeping them in local thread storage instead of sharing
/// the connection across cores.
///
/// WARNING: [`set_data_dir`] **MUST** be called and set before using [`Persister`].
#[derive(Debug)]
pub struct Persister;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Sets the data directory static for the thread local DB.
///
/// Must be called before accessing [`Persister`].
///
/// Can only be set once - will return whether value was set.
pub fn set_data_dir(data_dir: PathBuf) -> bool {
    DATA_DIR.set(data_dir).is_ok()
}

thread_local! {
    static DB: Rc<RefCell<Connection>> = RefCell::new(Connection::open(SignetWallet::db_path("default", DATA_DIR.get().expect("data dir to be set"))).unwrap()).into();
}

impl Persister {
    fn db() -> Rc<RefCell<Connection>> {
        DB.with(|db| db.clone())
    }
}

impl WalletPersister for Persister {
    type Error = rusqlite::Error;

    fn initialize(_persister: &mut Self) -> Result<bdk_wallet::ChangeSet, Self::Error> {
        let db = Self::db();
        let mut db_ref = db.borrow_mut();
        let db_tx = db_ref.transaction()?;
        ChangeSet::init_sqlite_tables(&db_tx)?;
        let changeset = ChangeSet::from_sqlite(&db_tx)?;
        db_tx.commit()?;
        Ok(changeset)
    }

    fn persist(
        _persister: &mut Self,
        changeset: &bdk_wallet::ChangeSet,
    ) -> Result<(), Self::Error> {
        let db = Self::db();
        let mut db_ref = db.borrow_mut();
        let db_tx = db_ref.transaction()?;
        changeset.persist_to_sqlite(&db_tx)?;
        db_tx.commit()
    }
}
