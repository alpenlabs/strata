use argon2::{Algorithm, Argon2, Params, Version};
use dialoguer::Password as InputPassword;
use zeroize::ZeroizeOnDrop;
use zxcvbn::{zxcvbn, Entropy};

use super::PW_SALT_LEN;

#[derive(ZeroizeOnDrop)]
pub struct Password {
    inner: String,
}

pub enum HashVersion {
    V0,
}

impl HashVersion {
    const fn params(&self) -> (Algorithm, Version, Result<Params, argon2::Error>) {
        match self {
            HashVersion::V0 => (
                Algorithm::Argon2id,
                Version::V0x13,
                // NOTE: This is the OWASP recommended params for Argon2id
                //       see https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html#argon2id
                Params::new(19_456, 2, 1, Some(32)),
            ),
        }
    }
}

impl Password {
    pub fn read(new: bool) -> Result<Self, dialoguer::Error> {
        let mut input = InputPassword::new().allow_empty_password(true);
        if new {
            input = input
                .with_prompt("Create a new password (leave empty for no password, dangerous!)")
                .with_confirmation(
                    "Confirm password (leave empty for no password, dangerous!)",
                    "Passwords didn't match",
                );
        } else {
            input = input.with_prompt("Enter your password");
        }

        let password = input.interact()?;

        Ok(Self { inner: password })
    }

    /// Returns the password entropy.
    pub fn entropy(&self) -> Entropy {
        zxcvbn(self.inner.as_str(), &[])
    }

    pub fn seed_encryption_key(
        &mut self,
        salt: &[u8; PW_SALT_LEN],
        version: HashVersion,
    ) -> Result<[u8; 32], argon2::Error> {
        let mut sek = [0u8; 32];
        let (algo, ver, params) = version.params();
        Argon2::new(algo, ver, params.expect("valid params")).hash_password_into(
            self.inner.as_bytes(),
            salt,
            &mut sek,
        )?;
        Ok(sek)
    }
}

#[derive(Debug)]
pub struct IncorrectPassword;
