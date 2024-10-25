pub mod backend;
pub mod persist;

use std::{
    fmt::Debug,
    io::{self},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::Arc,
};

use backend::{ScanError, SignetBackend, SyncError};
use bdk_esplora::esplora_client::{self, AsyncClient};
use bdk_wallet::{
    bitcoin::{FeeRate, Network, Txid},
    rusqlite::{self, Connection},
    PersistedWallet,
};
use console::{style, Term};
use persist::Persister;
use terrors::OneOf;

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
/// A wrapper around BDK's wallet with some custom logic
pub struct SignetWallet {
    wallet: PersistedWallet<Persister>,
    sync_backend: Arc<dyn SignetBackend>,
}

impl SignetWallet {
    fn db_path(wallet: &str, data_dir: &Path) -> PathBuf {
        data_dir.join(wallet).with_extension("sqlite")
    }

    pub fn persister(data_dir: &Path) -> Result<Connection, rusqlite::Error> {
        Connection::open(Self::db_path("default", data_dir))
    }

    pub fn new(
        seed: &Seed,
        network: Network,
        sync_backend: Arc<dyn SignetBackend>,
    ) -> io::Result<Self> {
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

    pub async fn sync(&mut self) -> Result<(), OneOf<(SyncError, rusqlite::Error)>> {
        self.sync_backend
            .sync_wallet(&mut self.wallet)
            .await
            .map_err(OneOf::new)?;
        self.persist().map_err(OneOf::new)?;
        Ok(())
    }
    pub async fn scan(&mut self) -> Result<(), OneOf<(ScanError, rusqlite::Error)>> {
        self.sync_backend
            .scan_wallet(&mut self.wallet)
            .await
            .map_err(OneOf::new)?;
        self.persist().map_err(OneOf::new)?;
        Ok(())
    }

    pub fn persist(&mut self) -> Result<bool, rusqlite::Error> {
        self.wallet.persist(&mut Persister)
    }
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
