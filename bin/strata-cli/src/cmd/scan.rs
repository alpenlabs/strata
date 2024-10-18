use argh::FromArgs;

use crate::{seed::Seed, settings::Settings, signet::SignetWallet};

/// Performs a full scan of the signet wallet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "scan")]
pub struct ScanArgs {}

pub async fn scan(_args: ScanArgs, seed: Seed, settings: Settings) {
    let mut l1w =
        SignetWallet::new(&seed, settings.network, settings.sync_backend.clone()).unwrap();
    l1w.scan().await.unwrap();
}
