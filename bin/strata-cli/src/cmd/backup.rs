use argh::FromArgs;
use bip39::Language;

use crate::{
    errors::{user_err, CliError, UserInputError},
    seed::Seed,
};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "backup")]
/// Prints a BIP39 mnemonic encoding the internal wallet's seed bytes
pub struct BackupArgs {
    /// select a language for the BIP39 mnemonic. Defaults to English.
    /// Options:
    /// en, cn, cn-trad,
    /// cz, fr, it, jp, kr or es
    #[argh(option)]
    language: Option<String>,
}

pub async fn backup(args: BackupArgs, seed: Seed) -> Result<(), CliError> {
    let language = match args.language {
        Some(s) => s,
        None => "en".to_owned(),
    };
    let language = match language.as_str() {
        "en" => Language::English,
        "cn" => Language::SimplifiedChinese,
        "cn-trad" => Language::TraditionalChinese,
        "cz" => Language::Czech,
        "fr" => Language::French,
        "it" => Language::Italian,
        "jp" => Language::Japanese,
        "kr" => Language::Korean,
        "es" => Language::Spanish,
        _ => return Err(user_err(UserInputError::UnsupportedLanguage)),
    };
    seed.print_mnemonic(language);
    Ok(())
}
