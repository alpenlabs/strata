use argh::FromArgs;
use console::Term;
use rand_core::OsRng;

use crate::seed::{password::Password, EncryptedSeedPersister, Seed};

/// Changes the seed's encryption password
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "change-password")]
pub struct ChangePwdArgs {}

pub async fn change_pwd(_args: ChangePwdArgs, seed: Seed, persister: impl EncryptedSeedPersister) {
    let term = Term::stdout();
    let mut new_pw = Password::read(true).unwrap();
    let password_validation: Result<(), String> = new_pw.validate();
    if let Err(feedback) = password_validation {
        let _ = term.write_line(&format!("Password is weak. {}", feedback));
    }
    let encrypted_seed = seed.encrypt(&mut new_pw, &mut OsRng).unwrap();
    persister.save(&encrypted_seed).unwrap();
    let _ = term.write_line("Password changed successfully");
}
