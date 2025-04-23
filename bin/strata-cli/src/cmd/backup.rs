use argh::FromArgs;
use bip39::Language;
use terrors::OneOf;

use crate::{errors::UnsupportedLanguage, handle_or_exit, seed::Seed};

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

/// Errors that can occur when printing the BIP39 mnemonic
pub(crate) type BackupError = OneOf<(UnsupportedLanguage,)>;

pub async fn backup(args: BackupArgs, seed: Seed) {
    handle_or_exit!(backup_inner(args, seed).await);
}

async fn backup_inner(args: BackupArgs, seed: Seed) -> Result<(), BackupError> {
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
        _ => return Err(OneOf::new(UnsupportedLanguage(language)))?,
    };
    seed.print_mnemonic(language);
    Ok(())
}
