use argh::FromArgs;
use colored::Colorize;
use dialoguer::Confirm;

use crate::{seed::EncryptedSeedPersister, settings::Settings};

/// DANGER: resets the CLI completely, destroying all keys and databases.
/// Keeps config.
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "reset")]
pub struct ResetArgs {
    /// dangerous: permit to reset without further confirmation
    #[argh(switch, short = 'y')]
    assume_yes: bool,
}

pub async fn reset(args: ResetArgs, persister: impl EncryptedSeedPersister, settings: Settings) {
    let confirm = if args.assume_yes {
        true
    } else {
        println!("{}", "This will DESTROY ALL DATA.".to_string().red().bold());
        Confirm::new()
            .with_prompt("Do you REALLY want to continue?")
            .interact()
            .unwrap()
    };

    if confirm {
        persister.delete().unwrap();
        println!("Wiped seed");
        std::fs::remove_dir_all(settings.data_dir.clone()).unwrap();
        println!("Wiped data directory");
    }
}
