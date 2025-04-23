#[cfg(target_os = "linux")]
use std::io;

use argh::FromArgs;
use rand_core::OsRng;
use terrors::OneOf;

use crate::{
    errors::{NoStorageAccess, PlatformFailure},
    handle_or_exit,
    seed::{password::Password, EncryptedSeedPersister, Seed},
};

/// Changes the seed's encryption password
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "change-password")]
pub struct ChangePwdArgs {}

/// Errors that can occur when changing seed encryption password
#[cfg(target_os = "linux")]
pub(crate) type ChangePasswordError = OneOf<(io::Error, dialoguer::Error, argon2::Error)>;

#[cfg(not(target_os = "linux"))]
pub(crate) type ChangePasswordError = OneOf<(
    PlatformFailure,
    NoStorageAccess,
    dialoguer::Error,
    argon2::Error,
)>;

pub async fn change_pwd(_args: ChangePwdArgs, seed: Seed, persister: impl EncryptedSeedPersister) {
    handle_or_exit!(change_pwd_inner(_args, seed, persister).await);
}

async fn change_pwd_inner(
    _args: ChangePwdArgs,
    seed: Seed,
    persister: impl EncryptedSeedPersister,
) -> Result<(), ChangePasswordError> {
    let mut new_pw = Password::read(true).map_err(OneOf::new)?;
    if let Err(feedback) = new_pw.validate() {
        println!("Password is weak. {}", feedback);
    }
    let encrypted_seed = match seed.encrypt(&mut new_pw, &mut OsRng) {
        Ok(es) => es,
        Err(e) => {
            let narrowed = e.narrow::<aes_gcm_siv::Error, _>();
            if let Ok(aes_error) = narrowed {
                panic!("Failed to encrypt seed: {aes_error:?}");
            }

            return Err(narrowed.unwrap_err().broaden());
        }
    };
    persister.save(&encrypted_seed).map_err(OneOf::broaden)?;

    println!("Password changed successfully");
    Ok(())
}
