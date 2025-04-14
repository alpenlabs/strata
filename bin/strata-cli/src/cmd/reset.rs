use argh::FromArgs;
use colored::Colorize;
use dialoguer::Confirm;

use crate::{errors::CliError, seed::EncryptedSeedPersister, settings::Settings};

/// DANGER: resets the CLI completely, destroying all keys and databases.
/// Keeps config.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "reset")]
pub struct ResetArgs {
    /// dangerous: permit to reset without further confirmation
    #[argh(switch, short = 'y')]
    assume_yes: bool,
}

pub async fn reset(
    args: ResetArgs,
    persister: impl EncryptedSeedPersister,
    settings: Settings,
) -> Result<(), CliError> {
    let confirm = if args.assume_yes {
        true
    } else {
        println!("{}", "This will DESTROY ALL DATA.".to_string().red().bold());
        Confirm::new()
            .with_prompt("Do you REALLY want to continue?")
            .interact()
            .map_err(|e| {
                CliError::Internal(anyhow::anyhow!(
                    "failed to read reset confirmation: {:?}",
                    e
                ))
            })?
    };

    if confirm {
        persister
            .delete()
            .map_err(|e| CliError::Internal(anyhow::anyhow!("failed to delete seed: {:?}", e)))?;
        println!("Wiped seed");
        std::fs::remove_dir_all(settings.data_dir.clone()).map_err(|e| {
            CliError::Internal(anyhow::anyhow!("failed to delete data deirectory: {:?}", e))
        })?;
        println!("Wiped data directory");
    }

    Ok(())
}
