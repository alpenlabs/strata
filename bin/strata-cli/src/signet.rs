use std::{
    cell::RefCell,
    collections::BTreeSet,
    io::{self},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use bdk_bitcoind_rpc::{bitcoincore_rpc, Emitter};
use bdk_esplora::{
    esplora_client::{self, AsyncClient},
    EsploraAsyncExt,
};
use bdk_wallet::{
    bitcoin::{Block, FeeRate, Network, Transaction, Txid},
    rusqlite::{self, Connection},
    ChangeSet, KeychainKind, PersistedWallet, WalletPersister,
};
use console::{style, Term};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::mpsc;

use crate::{seed::Seed, settings::Settings};

pub fn log_fee_rate(term: &Term, fr: &FeeRate) {
    let _ = term.write_line(&format!(
        "Using {} as feerate",
        style(format!("{} sat/vb", fr.to_sat_per_vb_ceil())).green(),
    ));
}

pub fn print_explorer_url(txid: &Txid, term: &Term, settings: &Settings) -> Result<(), io::Error> {
    term.write_line(&format!(
        "View transaction at {}",
        style(format!("{}/tx/{txid}", settings.mempool_endpoint)).blue()
    ))
}

#[derive(Debug)]
pub enum FeeRateError {
    InvalidDueToOverflow,
    BelowBroadcastMin,
    /// Esplora didn't have a fee for the requested target
    FeeMissing,
    EsploraError(esplora_client::Error),
}

pub async fn get_fee_rate(
    user_provided: Option<u64>,
    esplora: &EsploraClient,
    target: u16,
) -> Result<FeeRate, FeeRateError> {
    let fee_rate = if let Some(fr) = user_provided {
        FeeRate::from_sat_per_vb(fr).ok_or(FeeRateError::InvalidDueToOverflow)?
    } else {
        esplora
            .get_fee_estimates()
            .await
            .map(|frs| frs.get(&target).cloned())
            .map_err(FeeRateError::EsploraError)?
            .and_then(|fr| FeeRate::from_sat_per_vb(fr as u64))
            .ok_or(FeeRateError::FeeMissing)?
    };

    if fee_rate < FeeRate::BROADCAST_MIN {
        Err(FeeRateError::BelowBroadcastMin)
    } else {
        Ok(fee_rate)
    }
}

#[derive(Clone, Debug)]
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

#[derive(Debug)]
pub enum SyncBackend {
    Esplora(EsploraClient),
    BitcoinCore(bitcoincore_rpc::Client),
}

#[derive(Debug)]
/// A wrapper around BDK's wallet with some custom logic
pub struct SignetWallet {
    wallet: PersistedWallet<Persister>,
    sync_backend: Arc<SyncBackend>,
}

impl SignetWallet {
    fn db_path(wallet: &str, data_dir: &Path) -> PathBuf {
        data_dir.join(wallet).with_extension("sqlite")
    }

    pub fn persister(data_dir: &Path) -> Result<Connection, rusqlite::Error> {
        Connection::open(Self::db_path("default", data_dir))
    }

    pub fn new(seed: &Seed, network: Network, sync_backend: Arc<SyncBackend>) -> io::Result<Self> {
        let (load, create) = seed.signet_wallet().split();
        Ok(Self {
            wallet: load
                .check_network(network)
                .load_wallet(&mut Persister)
                .expect("should be able to load wallet")
                .unwrap_or_else(|| {
                    create
                        .network(network)
                        .create_wallet(&mut Persister)
                        .expect("wallet creation to succeed")
                }),
            sync_backend,
        })
    }

    pub async fn sync(&mut self) -> Result<(), Box<esplora_client::Error>> {
        match *self.sync_backend {
            SyncBackend::Esplora(_) => self.sync_with_esplora().await,
            SyncBackend::BitcoinCore(_) => Ok(self
                .sync_core_inner(self.latest_checkpoint().height())
                .await),
        }
    }

    pub async fn scan(&mut self) -> Result<(), Box<esplora_client::Error>> {
        match *self.sync_backend {
            SyncBackend::Esplora(_) => self.scan_with_esplora().await,
            SyncBackend::BitcoinCore(_) => Ok(self.sync_core_inner(0).await),
        }
    }

    async fn sync_with_esplora(&mut self) -> Result<(), Box<esplora_client::Error>> {
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

        let SyncBackend::Esplora(ref esplora) = *self.sync_backend else {
            panic!("sync backend wasn't esplora")
        };
        let update = esplora.sync(req, 3).await?;
        ops2.finish();
        spks2.finish();
        txids2.finish();
        let _ = term.write_line("Persisting updates");
        self.apply_update(update)
            .expect("should be able to connect to db");
        self.persist().expect("persist should work");
        let _ = term.write_line("Wallet synced");
        Ok(())
    }

    async fn scan_with_esplora(&mut self) -> Result<(), Box<esplora_client::Error>> {
        let bar = ProgressBar::new_spinner();
        bar.enable_steady_tick(Duration::from_millis(100));
        let bar2 = bar.clone();
        let req = self
            .start_full_scan()
            .inspect({
                let mut once = BTreeSet::<KeychainKind>::new();
                move |keychain, spk_i, script| {
                    if once.insert(keychain) {
                        bar2.println(format!("\nScanning keychain [{:?}]", keychain));
                    }
                    bar2.println(format!("- idx {spk_i}: {script}"));
                }
            })
            .build();

        let SyncBackend::Esplora(ref esplora) = *self.sync_backend else {
            panic!("sync backend wasn't esplora")
        };
        let update = esplora.full_scan(req, 5, 3).await?;
        bar.set_message("Persisting updates");
        self.apply_update(update)
            .expect("should be able to connect to db");
        self.persist().expect("persist should work");
        bar.finish_with_message("Scan complete");
        Ok(())
    }

    async fn sync_core_inner(&mut self, start_height: u32) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tip = self.latest_checkpoint();
        let bar = ProgressBar::new_spinner().with_style(
            ProgressStyle::with_template("{spinner} [{elapsed_precise}] {msg}").unwrap(),
        );
        bar.enable_steady_tick(Duration::from_millis(100));
        let bar2 = bar.clone();
        let mut blocks_scanned = 0;
        let sync_backend2 = self.sync_backend.clone();
        std::thread::spawn(move || {
            let SyncBackend::BitcoinCore(ref client) = *sync_backend2 else {
                panic!("sync backend wasn't bitcoin core")
            };
            let mut emitter = Emitter::new(client, tip, start_height);
            while let Some(emission) = emitter.next_block().unwrap() {
                tx.send(Emission::Block(emission))
                    .expect("block should send");
            }
            bar2.println("Scanning mempool");
            tx.send(Emission::Mempool(emitter.mempool().unwrap()))
                .expect("mempool should send");
        });

        let mut mempool_txs_len = 0;

        loop {
            if let Some(em) = rx.recv().await {
                match em {
                    Emission::Block(ev) => {
                        blocks_scanned += 1;
                        let height = ev.block_height();
                        let hash = ev.block_hash();
                        let connected_to = ev.connected_to();
                        let start_apply_block = Instant::now();
                        self.apply_block_connected_to(&ev.block, height, connected_to)
                            .expect("applying block should succeed");
                        let elapsed = start_apply_block.elapsed();
                        bar.println(format!(
                            "Applied block {} at height {} in {:?}",
                            hash, height, elapsed
                        ));
                        bar.set_message(format!(
                            "Current height: {}, scanned {} blocks",
                            height, blocks_scanned
                        ))
                    }
                    Emission::Mempool(txs) => {
                        bar.println("Scanning mempool");
                        let apply_start = Instant::now();
                        mempool_txs_len = txs.len();
                        self.apply_unconfirmed_txs(txs);
                        let elapsed = apply_start.elapsed();
                        bar.println(format!(
                            "Applied {} unconfirmed transactions in {:?}",
                            mempool_txs_len, elapsed
                        ));
                        break;
                    }
                }
            } else {
                break;
            }
        }

        bar.println("Persisting updates");
        self.persist().expect("persist to succeed");
        bar.finish_with_message(format!(
            "Completed sync. {} new blocks, {} unconfirmed transactions",
            blocks_scanned, mempool_txs_len
        ))
    }

    pub fn persist(&mut self) -> Result<bool, rusqlite::Error> {
        self.wallet.persist(&mut Persister)
    }
}

#[derive(Debug)]
enum Emission {
    Block(bdk_bitcoind_rpc::BlockEvent<Block>),
    Mempool(Vec<(Transaction, u64)>),
}

impl Deref for SignetWallet {
    type Target = PersistedWallet<Persister>;

    fn deref(&self) -> &Self::Target {
        &self.wallet
    }
}

impl DerefMut for SignetWallet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.wallet
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
