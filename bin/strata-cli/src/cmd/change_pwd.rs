use argh::FromArgs;
use console::Term;
use rand::rngs::OsRng;
use zxcvbn::Score;

use crate::seed::{password::Password, EncryptedSeedPersister, Seed};

/// Changes the seed's encryption password
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "change-password")]
pub struct ChangePwdArgs {}

pub async fn change_pwd(_args: ChangePwdArgs, seed: Seed, persister: impl EncryptedSeedPersister) {
    let term = Term::stdout();
    let mut new_pw = Password::read(true).unwrap();
    let entropy = new_pw.entropy();
    let _ = term.write_line(format!("Password strength (Overall strength score from 0-4, where anything below 3 is too weak): {}", entropy.score()).as_str());
    if entropy.score() <= Score::Two {
        let _ = term.write_line(
            entropy
                .feedback()
                .expect("No feedback")
                .to_string()
                .as_str(),
        );
    }
    let encrypted_seed = seed.encrypt(&mut new_pw, &mut OsRng).unwrap();
    persister.save(&encrypted_seed).unwrap();
    let _ = term.write_line("Password changed successfully");
}
