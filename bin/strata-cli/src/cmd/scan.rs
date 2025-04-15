use argh::FromArgs;
use terrors::OneOf;

use crate::{
    errors::{InternalError, UserInputError},
    seed::Seed,
    settings::Settings,
    signet::SignetWallet,
};

/// Performs a full scan of the signet wallet
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "scan")]
pub struct ScanArgs {}

pub async fn scan(
    _args: ScanArgs,
    seed: Seed,
    settings: Settings,
) -> Result<(), OneOf<(InternalError, UserInputError)>> {
    let mut l1w = SignetWallet::new(&seed, settings.network, settings.signet_backend.clone())
        .map_err(|e| OneOf::new(InternalError::LoadSignetWallet(format!("{e:?}"))))?;
    l1w.scan()
        .await
        .map_err(|e| OneOf::new(InternalError::ScanSignetWallet(format!("{e:?}"))))?;

    Ok(())
}
