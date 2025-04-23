use argh::FromArgs;
use colored::Colorize;
use dialoguer::Confirm;
use terrors::OneOf;

#[cfg(not(target_os = "linux"))]
use crate::errors::{NoStorageAccess, PlatformFailure};
use crate::{handle_or_exit, seed::EncryptedSeedPersister, settings::Settings};

/// DANGER: resets the CLI completely, destroying all keys and databases.
/// Keeps config.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "reset")]
pub struct ResetArgs {
    /// dangerous: permit to reset without further confirmation
    #[argh(switch, short = 'y')]
    assume_yes: bool,
}

/// Errors that can occur when resetting the CLI
#[cfg(target_os = "linux")]
pub(crate) type ResetError = OneOf<(std::io::Error, dialoguer::Error, argon2::Error)>;

#[cfg(not(target_os = "linux"))]
pub(crate) type ResetError = OneOf<(
    PlatformFailure,
    NoStorageAccess,
    dialoguer::Error,
    std::io::Error,
)>;

pub async fn reset(args: ResetArgs, persister: impl EncryptedSeedPersister, settings: Settings) {
    handle_or_exit!(reset_inner(args, persister, settings).await);
}

async fn reset_inner(
    args: ResetArgs,
    persister: impl EncryptedSeedPersister,
    settings: Settings,
) -> Result<(), ResetError> {
    let confirm = if args.assume_yes {
        true
    } else {
        println!("{}", "This will DESTROY ALL DATA.".to_string().red().bold());
        Confirm::new()
            .with_prompt("Do you REALLY want to continue?")
            .interact()
            .map_err(OneOf::new)?
    };

    if confirm {
        persister.delete().map_err(OneOf::broaden)?;
        println!("Wiped seed");
        std::fs::remove_dir_all(settings.data_dir.clone()).map_err(OneOf::new)?;
        println!("Wiped data directory");
    }

    Ok(())
}
