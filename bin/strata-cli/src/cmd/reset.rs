use argh::FromArgs;
use console::Term;
use dialoguer::Confirm;

use crate::seed;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "reset")]
/// Prints a BIP39 mnemonic encoding the internal wallet's seed bytes
pub struct ResetArgs {
    #[argh(switch, short = 'y')]
    /// select a language for the BIP39 mnemonic. Defaults to English.
    /// Options:
    /// english, chinese-simplified, chinese-traditional,
    /// czech, french, italian, japanese, korean,
    /// portuguese or spanish
    assume_yes: bool,
}

pub async fn reset(args: ResetArgs) {
    let term = Term::stdout();
    let confirm = if args.assume_yes {
        true
    } else {
        let _ = term.write_line("This will DESTROY ALL DATA.");
        Confirm::new()
            .with_prompt("Do you really want to continue?")
            .interact()
            .unwrap()
    };

    if confirm {
        seed::reset().unwrap();
    }
}
