use argh::FromArgs;
use console::Term;
use rand::thread_rng;

use crate::seed::{password::Password, EncryptedSeedPersister, Seed};

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "change-password")]
/// Changes the seed's encryption password
pub struct ChangePwdArgs {}

pub async fn change_pwd(_args: ChangePwdArgs, seed: Seed, persister: impl EncryptedSeedPersister) {
    let term = Term::stdout();
    let mut new_pw = Password::read(true).unwrap();
    let encrypted_seed = seed.encrypt(&mut new_pw, &mut thread_rng()).unwrap();
    persister.save(&encrypted_seed).unwrap();
    let _ = term.write_line("Password changed successfully");
}
