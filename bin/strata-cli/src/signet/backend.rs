use std::{
    collections::BTreeSet,
    fmt::Debug,
    marker::Send,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
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

use super::EsploraClient;

macro_rules! boxed_err {
    ($name:ident) => {
        impl std::ops::Deref for $name {
            type Target = dyn Debug;

            fn deref(&self) -> &Self::Target {
                self.0.as_ref()
            }
        }

        impl From<Box<dyn Debug>> for $name {
            fn from(err: Box<dyn Debug>) -> Self {
                Self(err)
            }
        }
    };
}

#[derive(Debug)]
pub struct SyncError(Box<dyn Debug>);
boxed_err!(SyncError);

#[derive(Debug)]
pub struct ScanError(Box<dyn Debug>);
boxed_err!(ScanError);

#[derive(Debug)]
pub struct BroadcastTxError(Box<dyn Debug>);
boxed_err!(BroadcastTxError);

#[derive(Debug)]
pub struct GetFeeRateError(Box<dyn Debug>);
boxed_err!(GetFeeRateError);

#[async_trait]
pub trait SignetBackend: Debug {
    async fn sync_wallet(&self, wallet: &mut Wallet) -> Result<(), SyncError>;
    async fn scan_wallet(&self, wallet: &mut Wallet) -> Result<(), ScanError>;
    async fn broadcast_tx(&self, tx: &Transaction) -> Result<(), BroadcastTxError>;
    async fn get_fee_rate(
        &self,
        target: u16,
    ) -> Result<FeeRate, OneOf<(InvalidFee, GetFeeRateError)>>;
}

#[async_trait]
impl SignetBackend for EsploraClient {
    async fn sync_wallet(&self, wallet: &mut Wallet) -> Result<(), SyncError> {
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

        let update = self
            .sync(req, 3)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Debug>)?;
        ops2.finish();
        spks2.finish();
        txids2.finish();
        let _ = term.write_line("Updating wallet");
        wallet
            .apply_update(update)
            .expect("should be able to connect to db");
        let _ = term.write_line("Wallet synced");
        Ok(())
    }

    async fn scan_wallet(&self, wallet: &mut Wallet) -> Result<(), ScanError> {
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

        let update = self
            .full_scan(req, 5, 3)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Debug>)?;
        bar.set_message("Persisting updates");
        wallet
            .apply_update(update)
            .expect("should be able to connect to db");
        bar.finish_with_message("Scan complete");
        Ok(())
    }

    async fn broadcast_tx(&self, tx: &Transaction) -> Result<(), BroadcastTxError> {
        self.broadcast(tx)
            .await
            .map_err(|e| (Box::new(e) as Box<dyn Debug>).into())
    }

    async fn get_fee_rate(
        &self,
        target: u16,
    ) -> Result<FeeRate, OneOf<(InvalidFee, GetFeeRateError)>> {
        match self
            .get_fee_estimates()
            .await
            .map(|frs| frs.get(&target).cloned())
            .map_err(|e| GetFeeRateError(Box::new(e) as Box<dyn Debug>))
            .map_err(OneOf::new)?
            .and_then(|fr| FeeRate::from_sat_per_vb(fr as u64))
        {
            Some(fr) => Ok(fr),
            None => Err(OneOf::new(InvalidFee)),
        }
    }
}

#[async_trait]
impl SignetBackend for Arc<bitcoincore_rpc::Client> {
    async fn sync_wallet(&self, wallet: &mut Wallet) -> Result<(), SyncError> {
        sync_wallet_with_core(self.clone(), wallet, false)
            .await
            .map_err(|e| (Box::new(e) as Box<dyn Debug>).into())
    }

    async fn scan_wallet(&self, wallet: &mut Wallet) -> Result<(), ScanError> {
        sync_wallet_with_core(self.clone(), wallet, true)
            .await
            .map_err(|e| (Box::new(e) as Box<dyn Debug>).into())
    }

    async fn broadcast_tx(&self, tx: &Transaction) -> Result<(), BroadcastTxError> {
        let hex = encode::serialize_hex(tx);

        spawn_bitcoin_core(self.clone(), move |c| c.send_raw_transaction(hex))
            .await
            .map_err(|e| BroadcastTxError(Box::new(e) as Box<dyn Debug>))?;
        Ok(())
    }

    async fn get_fee_rate(
        &self,
        target: u16,
    ) -> Result<FeeRate, OneOf<(InvalidFee, GetFeeRateError)>> {
        let res = spawn_bitcoin_core(self.clone(), move |c| {
            c.estimate_smart_fee(target, Some(EstimateMode::Conservative))
        })
        .await
        .map_err(|e| GetFeeRateError(Box::new(e) as Box<dyn Debug>))
        .map_err(OneOf::new)?;

        res.fee_rate
            .and_then(|fr| {
                let per_vb = (fr / 1000).to_sat();
                FeeRate::from_sat_per_vb(per_vb)
            })
            .ok_or(OneOf::new(InvalidFee))
    }
}

async fn spawn_bitcoin_core<T, F>(
    client: Arc<bitcoincore_rpc::Client>,
    func: F,
) -> Result<T, bitcoincore_rpc::Error>
where
    T: Send + 'static,
    F: FnOnce(&bitcoincore_rpc::Client) -> Result<T, bitcoincore_rpc::Error> + Send + 'static,
{
    let handle = tokio::task::spawn_blocking(move || func(&client));
    handle.await.expect("thread should be fine")
}

async fn sync_wallet_with_core(
    client: Arc<bitcoincore_rpc::Client>,
    wallet: &mut Wallet,
    should_scan: bool,
) -> Result<(), bitcoincore_rpc::Error> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let last_cp = wallet.latest_checkpoint();
    let bar = ProgressBar::new_spinner()
        .with_style(ProgressStyle::with_template("{spinner} [{elapsed_precise}] {msg}").unwrap());
    bar.enable_steady_tick(Duration::from_millis(100));
    let bar2 = bar.clone();
    let mut blocks_scanned = 0;

    let start_height = match should_scan {
        true => 0,
        false => last_cp.height(),
    };

    let mut handle = Box::pin(spawn_bitcoin_core(client.clone(), move |client| {
        let mut emitter = Emitter::new(client, last_cp, start_height);
        while let Some(emission) = emitter.next_block().unwrap() {
            tx.send(Emission::Block(emission))
                .expect("block should send");
        }
        bar2.println("Scanning mempool");
        tx.send(Emission::Mempool(emitter.mempool().unwrap()))
            .expect("mempool should send");
        Ok(())
    }));

    let mempool_txs_len = &mut 0usize;
    loop {
        tokio::select! {
            biased;
            Err(e) = handle.as_mut() => return Err(e),
            Some(em) = rx.recv() => {
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
                        *mempool_txs_len = txs.len();
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
        }
    }

    bar.println("Persisting updates");
    bar.finish_with_message(format!(
        "Completed sync. {} new blocks, {} unconfirmed transactions",
        blocks_scanned, mempool_txs_len
    ));
    Ok(())
}

#[derive(Debug)]
pub struct InvalidFee;

#[derive(Debug)]
struct BelowBroadcastMin;

#[derive(Debug)]
struct FeeMissing;

#[derive(Debug)]
pub enum FeeRateError {
    InvalidDueToOverflow,
    /// The fee obtained, either from a backend or provided by the user, was
    /// below bitcoin's minimum fee rate
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
