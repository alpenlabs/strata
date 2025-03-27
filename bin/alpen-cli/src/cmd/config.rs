use argh::FromArgs;

use crate::settings::CONFIG_FILE;

/// Prints the location of the CLI's TOML config file
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "config")]
pub struct ConfigArgs {}

pub async fn config(_args: ConfigArgs) {
    println!("{}", CONFIG_FILE.to_string_lossy())
}
