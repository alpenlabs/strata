use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};

use bdk_bitcoind_rpc::{
    bitcoincore_rpc::{self, json::EstimateMode, RpcApi},
    Emitter,
};
use bdk_esplora::{esplora_client, EsploraAsyncExt};
use bdk_wallet::{
    bitcoin::{consensus::encode, Block, FeeRate, Transaction},
    KeychainKind, Wallet,
};
use console::Term;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use terrors::OneOf;
use tokio::sync::mpsc;

use super::{persist::WalletPersistWrapper, EsploraClient};

#[derive(Debug)]
enum SignetBackendInner {
    Esplora(EsploraClient),
    BitcoinCore(bitcoincore_rpc::Client),
}

#[derive(Debug, Clone)]
pub struct SignetBackend(Arc<SignetBackendInner>);

impl From<EsploraClient> for SignetBackend {
    fn from(value: EsploraClient) -> Self {
        SignetBackend(SignetBackendInner::Esplora(value).into())
    }
}

impl From<bitcoincore_rpc::Client> for SignetBackend {
    fn from(value: bitcoincore_rpc::Client) -> Self {
        SignetBackend(SignetBackendInner::BitcoinCore(value).into())
    }
}

impl SignetBackend {
    pub async fn sync_wallet(&self, wallet: &mut Wallet) -> Result<(), Box<esplora_client::Error>> {
        match *self.0 {
            SignetBackendInner::Esplora(_) => self.sync_with_esplora(wallet).await,
            SignetBackendInner::BitcoinCore(_) => {
                let start_height = wallet.latest_checkpoint().height();
                self.sync_with_core(wallet, start_height).await;
                Ok(())
            }
        }
    }

    pub async fn scan_wallet(&self, wallet: &mut Wallet) -> Result<(), Box<esplora_client::Error>> {
        match *self.0 {
            SignetBackendInner::Esplora(_) => self.scan_with_esplora(wallet).await,
            SignetBackendInner::BitcoinCore(_) => {
                self.sync_with_core(wallet, 0).await;
                Ok(())
            }
        }
    }

    async fn sync_with_esplora(
        &self,
        wallet: &mut Wallet,
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
        let req = wallet
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

        let SignetBackendInner::Esplora(ref esplora) = *self.0 else {
            panic!("sync backend wasn't esplora")
        };
        let update = esplora.sync(req, 3).await?;
        ops2.finish();
        spks2.finish();
        txids2.finish();
        let _ = term.write_line("Persisting updates");
        wallet
            .apply_update(update)
            .expect("should be able to connect to db");
        wallet.persist().expect("persist should work");
        let _ = term.write_line("Wallet synced");
        Ok(())
    }

    async fn scan_with_esplora(
        &self,
        wallet: &mut Wallet,
    ) -> Result<(), Box<esplora_client::Error>> {
        let bar = ProgressBar::new_spinner();
        bar.enable_steady_tick(Duration::from_millis(100));
        let bar2 = bar.clone();
        let req = wallet
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

        let SignetBackendInner::Esplora(ref esplora) = *self.0 else {
            panic!("sync backend wasn't esplora")
        };
        let update = esplora.full_scan(req, 5, 3).await?;
        bar.set_message("Persisting updates");
        wallet
            .apply_update(update)
            .expect("should be able to connect to db");
        wallet.persist().expect("persist should work");
        bar.finish_with_message("Scan complete");
        Ok(())
    }

    async fn sync_with_core(&self, wallet: &mut Wallet, start_height: u32) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tip = wallet.latest_checkpoint();
        let bar = ProgressBar::new_spinner().with_style(
            ProgressStyle::with_template("{spinner} [{elapsed_precise}] {msg}").unwrap(),
        );
        bar.enable_steady_tick(Duration::from_millis(100));
        let bar2 = bar.clone();
        let mut blocks_scanned = 0;
        let backend = self.0.clone();
        std::thread::spawn(move || {
            let SignetBackendInner::BitcoinCore(ref client) = *backend else {
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

        while let Some(em) = rx.recv().await {
            match em {
                Emission::Block(ev) => {
                    blocks_scanned += 1;
                    let height = ev.block_height();
                    let hash = ev.block_hash();
                    let connected_to = ev.connected_to();
                    let start_apply_block = Instant::now();
                    wallet
                        .apply_block_connected_to(&ev.block, height, connected_to)
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
                    wallet.apply_unconfirmed_txs(txs);
                    let elapsed = apply_start.elapsed();
                    bar.println(format!(
                        "Applied {} unconfirmed transactions in {:?}",
                        mempool_txs_len, elapsed
                    ));
                    break;
                }
            }
        }

        bar.println("Persisting updates");
        wallet.persist().expect("persist to succeed");
        bar.finish_with_message(format!(
            "Completed sync. {} new blocks, {} unconfirmed transactions",
            blocks_scanned, mempool_txs_len
        ))
    }

    pub async fn broadcast_tx(
        &self,
        tx: &Transaction,
    ) -> Result<(), OneOf<(esplora_client::Error, bitcoincore_rpc::Error)>> {
        match *self.0 {
            SignetBackendInner::Esplora(ref client) => {
                Ok(client.broadcast(tx).await.map_err(OneOf::new)?)
            }

            SignetBackendInner::BitcoinCore(_) => {
                let hex = encode::serialize_hex(tx);
                let backend = self.0.clone();
                let handle = tokio::task::spawn_blocking(move || {
                    let SignetBackendInner::BitcoinCore(ref client) = *backend else {
                        panic!("sync backend wasn't bitcoin core")
                    };
                    client.send_raw_transaction(hex)
                });
                handle
                    .await
                    .expect("thread should be fine")
                    .map_err(OneOf::new)?;
                Ok(())
            }
        }
    }

    pub async fn get_fee_rate(
        &self,
        fallback_in_sats: Option<u64>,
        target: u16,
    ) -> Result<FeeRate, FeeRateError> {
        let fee_rate = if let Some(fr) = fallback_in_sats {
            FeeRate::from_sat_per_vb(fr).ok_or(FeeRateError::InvalidDueToOverflow)?
        } else {
            match *self.0 {
                SignetBackendInner::Esplora(ref esplora) => esplora
                    .get_fee_estimates()
                    .await
                    .map(|frs| frs.get(&target).cloned())
                    .map_err(FeeRateError::EsploraError)?
                    .and_then(|fr| FeeRate::from_sat_per_vb(fr as u64))
                    .unwrap_or(FeeRate::BROADCAST_MIN),

                SignetBackendInner::BitcoinCore(_) => {
                    let backend = self.0.clone();
                    let handle = tokio::task::spawn_blocking(move || {
                        let SignetBackendInner::BitcoinCore(ref client) = *backend else {
                            panic!("sync backend wasn't bitcoin core")
                        };
                        client.estimate_smart_fee(target, Some(EstimateMode::Conservative))
                    });
                    let res = handle
                        .await
                        .expect("thread should be fine")
                        .map_err(FeeRateError::BitcoinCoreRPCError)?;
                    let per_vb =
                        (res.fee_rate.expect("fee rate should be present") / 1000).to_sat();
                    FeeRate::from_sat_per_vb(per_vb).ok_or(FeeRateError::InvalidDueToOverflow)?
                }
            }
        };

        if fee_rate < FeeRate::BROADCAST_MIN {
            Err(FeeRateError::BelowBroadcastMin)
        } else {
            Ok(fee_rate)
        }
    }
}

#[derive(Debug)]
pub enum FeeRateError {
    InvalidDueToOverflow,
    BelowBroadcastMin,
    /// Esplora didn't have a fee for the requested target
    FeeMissing,
    EsploraError(esplora_client::Error),
    BitcoinCoreRPCError(bitcoincore_rpc::Error),
}

#[derive(Debug)]
enum Emission {
    Block(bdk_bitcoind_rpc::BlockEvent<Block>),
    Mempool(Vec<(Transaction, u64)>),
}
