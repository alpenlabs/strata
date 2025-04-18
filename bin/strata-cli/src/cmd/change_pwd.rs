use argh::FromArgs;
use rand_core::OsRng;

use crate::{
    errors::{internal_err, CliError, InternalError},
    seed::{password::Password, EncryptedSeedPersister, Seed},
};

/// Changes the seed's encryption password
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "change-password")]
pub struct ChangePwdArgs {}

pub async fn change_pwd(
    _args: ChangePwdArgs,
    seed: Seed,
    persister: impl EncryptedSeedPersister,
) -> Result<(), CliError> {
    let mut new_pw = Password::read(true).map_err(internal_err(InternalError::ReadPassword))?;
    if let Err(feedback) = new_pw.validate() {
        println!("Password is weak. {}", feedback);
    }
    let encrypted_seed = seed
        .encrypt(&mut new_pw, &mut OsRng)
        .map_err(internal_err(InternalError::EncryptSeed))?;
    persister
        .save(&encrypted_seed)
        .map_err(internal_err(InternalError::PersistEncryptedSeed))?;

    println!("Password changed successfully");
    Ok(())
}
