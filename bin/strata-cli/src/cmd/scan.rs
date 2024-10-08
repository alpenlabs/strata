use argh::FromArgs;

use crate::{
    constants::NETWORK,
    seed::Seed,
    signet::{EsploraClient, SignetWallet},
};

/// Performs a full scan of the signet wallet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "scan")]
pub struct ScanArgs {}

pub async fn scan(_args: ScanArgs, seed: Seed, esplora: EsploraClient) {
    let mut l1w = SignetWallet::new(&seed, NETWORK).unwrap();
    l1w.scan(&esplora).await.unwrap();
}
