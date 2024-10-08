use argh::FromArgs;

use crate::settings::Settings;

/// Prints the location of the CLI's TOML config file
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "config")]
pub struct ConfigArgs {}

pub async fn config(_args: ConfigArgs, settings: Settings) {
    println!("{}", settings.config_file.to_string_lossy())
}
