pub mod backend;
pub mod persist;

use std::{
    fmt::Debug,
    io::{self},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::Arc,
};

use backend::{ScanError, SignetBackend, SyncError, WalletUpdate};
use bdk_esplora::esplora_client::{self, AsyncClient};
use bdk_wallet::{
    bitcoin::{FeeRate, Network, Txid},
    rusqlite::{self, Connection},
    PersistedWallet, Wallet,
};
use console::{style, Term};
use persist::Persister;
use terrors::OneOf;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::{seed::Seed, settings::Settings};

pub fn log_fee_rate(term: &Term, fr: &FeeRate) {
    let _ = term.write_line(&format!(
        "Using {} as feerate",
        style(format!("{} sat/vb", fr.to_sat_per_vb_ceil())).green(),
    ));
}

pub async fn get_fee_rate(
    user_provided_sats_per_vb: Option<u64>,
    signet_backend: &dyn SignetBackend,
) -> FeeRate {
    match user_provided_sats_per_vb {
        Some(fr) => FeeRate::from_sat_per_vb(fr).expect("valid fee rate"),
        None => signet_backend
            .get_fee_rate(1)
            .await
            .expect("valid fee rate")
            .unwrap_or(FeeRate::BROADCAST_MIN),
    }
}

pub fn print_bitcoin_explorer_url(
    txid: &Txid,
    term: &Term,
    settings: &Settings,
) -> Result<(), io::Error> {
    term.write_line(&match settings.mempool_space_endpoint {
        Some(ref url) => format!(
            "View transaction at {}",
            style(format!("{url}/tx/{txid}")).blue()
        ),
        None => format!("Transaction ID: {txid}"),
    })
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
        sync_wallet(&mut self.wallet, self.sync_backend.clone()).await?;
        self.persist().map_err(OneOf::new)?;
        Ok(())
    }

    pub async fn scan(&mut self) -> Result<(), OneOf<(ScanError, rusqlite::Error)>> {
        scan_wallet(&mut self.wallet, self.sync_backend.clone()).await?;
        self.persist().map_err(OneOf::new)?;
        Ok(())
    }

    pub fn persist(&mut self) -> Result<bool, rusqlite::Error> {
        self.wallet.persist(&mut Persister)
    }
}

pub async fn scan_wallet(
    wallet: &mut Wallet,
    sync_backend: Arc<dyn SignetBackend>,
) -> Result<(), OneOf<(ScanError, rusqlite::Error)>> {
    let req = wallet.start_full_scan();
    let last_cp = wallet.latest_checkpoint();
    let (tx, rx) = unbounded_channel();

    let handle = tokio::spawn(async move { sync_backend.scan_wallet(req, last_cp, tx).await });

    apply_update_stream(wallet, rx).await;

    handle
        .await
        .expect("thread to be fine")
        .map_err(OneOf::new)?;

    Ok(())
}

pub async fn sync_wallet(
    wallet: &mut Wallet,
    sync_backend: Arc<dyn SignetBackend>,
) -> Result<(), OneOf<(SyncError, rusqlite::Error)>> {
    let req = wallet.start_sync_with_revealed_spks();
    let last_cp = wallet.latest_checkpoint();
    let (tx, rx) = unbounded_channel();

    let handle = tokio::spawn(async move { sync_backend.sync_wallet(req, last_cp, tx).await });

    apply_update_stream(wallet, rx).await;

    handle
        .await
        .expect("thread to be fine")
        .map_err(OneOf::new)?;

    Ok(())
}

async fn apply_update_stream(wallet: &mut Wallet, mut rx: UnboundedReceiver<WalletUpdate>) {
    while let Some(update) = rx.recv().await {
        match update {
            WalletUpdate::SpkSync(update) => {
                wallet.apply_update(update).expect("update to connect")
            }
            WalletUpdate::SpkScan(update) => {
                wallet.apply_update(update).expect("update to connect")
            }
            WalletUpdate::NewBlock(ev) => {
                let height = ev.block_height();
                let connected_to = ev.connected_to();
                wallet
                    .apply_block_connected_to(&ev.block, height, connected_to)
                    .expect("block to be added")
            }
            WalletUpdate::MempoolTxs(txs) => wallet.apply_unconfirmed_txs(txs),
        }
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
