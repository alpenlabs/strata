use argh::FromArgs;

use crate::{errors::CliError, seed::Seed, settings::Settings, signet::SignetWallet};

/// Performs a full scan of the signet wallet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "scan")]
pub struct ScanArgs {}

pub async fn scan(_args: ScanArgs, seed: Seed, settings: Settings) -> Result<(), CliError> {
    let mut l1w = SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
        .map_err(|e| {
            CliError::Internal(anyhow::anyhow!("failed to load signet wallet: {:?}", e))
        })?;
    l1w.scan().await.map_err(|e| {
        CliError::Internal(anyhow::anyhow!("failed to scan signet wallet: {:?}", e))
    })?;

    Ok(())
}
