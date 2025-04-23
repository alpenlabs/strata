use argh::FromArgs;
use terrors::OneOf;

use crate::{
    errors::SignetWalletError, handle_or_exit, seed::Seed, settings::Settings, signet::SignetWallet,
};

/// Performs a full scan of the signet wallet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "scan")]
pub struct ScanArgs {}

/// Errors that can occur scanning signet wallet
pub(crate) type ScanError = OneOf<(SignetWalletError,)>;

pub async fn scan(_args: ScanArgs, seed: Seed, settings: Settings) {
    handle_or_exit!(scan_inner(_args, seed, settings).await);
}

async fn scan_inner(_args: ScanArgs, seed: Seed, settings: Settings) -> Result<(), ScanError> {
    let mut l1w = SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
        .map_err(|e| ScanError::new(SignetWalletError::new("Failed to load signet wallet", e)))?;
    l1w.scan()
        .await
        .map_err(|e| ScanError::new(SignetWalletError::new("Failed to scan signet wallet", e)))?;

    Ok(())
}
