use argh::FromArgs;
use bip39::Language;
use console::Term;

use crate::seed::Seed;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "backup")]
/// Prints a BIP39 mnemonic encoding the internal wallet's seed bytes
pub struct BackupArgs {
    #[argh(option)]
    /// select a language for the BIP39 mnemonic. Defaults to English.
    /// Options:
    /// en, cn, cn-trad,
    /// cz, fr, it, jp, kr or es
    language: Option<String>,
}

pub async fn backup(args: BackupArgs, seed: Seed) {
    let term = Term::stdout();
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
        _ => {
            let _ = term.write_line("invalid language. use --help to check available languages");
            std::process::exit(1);
        }
    };
    seed.print_mnemonic(language);
}
