use argon2::Argon2;
use dialoguer::Password as InputPassword;

use super::SALT_LEN;

pub struct Password {
    inner: String,
    seed_encryption_key: Option<[u8; 32]>,
}

impl Password {
    pub fn read(new: bool) -> Result<Self, dialoguer::Error> {
        let mut input = InputPassword::new();
        if new {
            input = input
                .with_prompt("Create a new password")
                .with_confirmation("Confirm password", "Passwords didn't match");
        } else {
            input = input.with_prompt("Enter your password");
        }

        let password = input.interact()?;

        Ok(Self {
            inner: password,
            seed_encryption_key: None,
        })
    }

    pub fn seed_encryption_key(
        &mut self,
        salt: &[u8; SALT_LEN],
    ) -> Result<&[u8; 32], argon2::Error> {
        match self.seed_encryption_key {
            Some(ref key) => Ok(key),
            None => {
                let mut sek = [0u8; 32];
                Argon2::default().hash_password_into(self.inner.as_bytes(), salt, &mut sek)?;
                self.seed_encryption_key = Some(sek);
                self.seed_encryption_key(salt)
            }
        }
    }
}
