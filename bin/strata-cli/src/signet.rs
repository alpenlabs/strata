use std::{
    io,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::LazyLock,
};

use bdk_esplora::{
    esplora_client::{self, AsyncClient},
    EsploraAsyncExt,
};
use bdk_wallet::{
    bitcoin::{FeeRate, Network},
    rusqlite::{self, Connection},
    PersistedWallet,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::{seed::BaseWallet, settings::SETTINGS};

const NETWORK: Network = Network::Signet;

/// Spawns a tokio task that updates the FEE_RATE every 20 seconds
pub async fn get_fee_rate() -> Result<Option<FeeRate>, esplora_client::Error> {
    Ok(ESPLORA_CLIENT
        .get_fee_estimates()
        .await
        .map(|frs| frs.get(&1).cloned())?
        .map(|fr| {
            (fr as u64)
                .checked_mul(1000 / 4)
                .map(FeeRate::from_sat_per_vb)
        })
        .flatten()
        .flatten())
}

/// Shared async client for esplora
pub static ESPLORA_CLIENT: LazyLock<AsyncClient> = LazyLock::new(|| {
    esplora_client::Builder::new(&SETTINGS.esplora)
        .build_async()
        .expect("valid esplora config")
});

#[derive(Debug)]
/// A wrapper around BDK's wallet with some custom logic
pub struct SignetWallet(PersistedWallet<Connection>);

impl SignetWallet {
    fn db_path(wallet: &str) -> PathBuf {
        SETTINGS.data_dir.join(wallet).with_extension("sqlite")
    }

    pub fn persister() -> Result<Connection, rusqlite::Error> {
        Connection::open(Self::db_path("default"))
    }

    pub fn new(base: BaseWallet) -> io::Result<Self> {
        let (load, create) = base.split();
        let mut db = Connection::open(Self::db_path("default")).unwrap();
        Ok(Self(
            load.check_network(NETWORK)
                .load_wallet(&mut db)
                .unwrap()
                .unwrap_or_else(|| {
                    create
                        .network(NETWORK)
                        .create_wallet(&mut db)
                        .expect("wallet creation to succeed")
                }),
        ))
    }

    pub async fn sync(&mut self) -> Result<(), Box<esplora_client::Error>> {
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

        let update = ESPLORA_CLIENT.sync(req, 3).await?;
        ops2.finish();
        spks2.finish();
        txids2.finish();
        self.apply_update(update)
            .expect("should be able to connect to db");
        let mut db = Connection::open(Self::db_path("default")).unwrap();
        self.persist(&mut db).expect("persist should work");
        Ok(())
    }
}

impl Deref for SignetWallet {
    type Target = PersistedWallet<Connection>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SignetWallet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
