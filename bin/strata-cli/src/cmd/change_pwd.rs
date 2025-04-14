use argh::FromArgs;
use rand_core::OsRng;

use crate::{
    errors::CliError,
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
    let mut new_pw = Password::read(true)
        .map_err(|e| CliError::Internal(anyhow::anyhow!("failed to read password: {:?}", e)))?;
    if let Err(feedback) = new_pw.validate() {
        println!("Password is weak. {}", feedback);
    }
    let encrypted_seed = seed
        .encrypt(&mut new_pw, &mut OsRng)
        .map_err(|e| CliError::Internal(anyhow::anyhow!("failed to encrypt seed: {:?}", e)))?;
    persister.save(&encrypted_seed).map_err(|e| {
        CliError::Internal(anyhow::anyhow!("failed to save encrypted seed: {:?}", e))
    })?;

    println!("Password changed successfully");
    Ok(())
}
