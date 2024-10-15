#[cfg(target_os = "linux")]
use std::io;
use std::str::FromStr;

use aes_gcm_siv::{aead::AeadMutInPlace, Aes256GcmSiv, KeyInit, Nonce, Tag};
use alloy::{network::EthereumWallet, signers::local::PrivateKeySigner};
use bdk_wallet::{
    bitcoin::{bip32::Xpriv, Network},
    CreateParams, KeychainKind, LoadParams, Wallet,
};
use bip39::{Language, Mnemonic};
use console::Term;
use dialoguer::{Confirm, Input};
use password::{HashVersion, IncorrectPassword, Password};
use rand::{rngs::OsRng, CryptoRng, RngCore};
use sha2::{Digest, Sha256};
use terrors::OneOf;
use zeroize::Zeroizing;

use crate::constants::{AES_NONCE_LEN, AES_TAG_LEN, PW_SALT_LEN, SEED_LEN};

pub struct BaseWallet(LoadParams, CreateParams);

impl BaseWallet {
    pub fn split(self) -> (LoadParams, CreateParams) {
        (self.0, self.1)
    }
}

#[derive(Clone)]
pub struct Seed(Zeroizing<[u8; SEED_LEN]>);

impl Seed {
    fn gen<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let mut bytes = Zeroizing::new([0u8; SEED_LEN]);
        rng.fill_bytes(bytes.as_mut());
        Self(bytes)
    }

    pub fn print_mnemonic(&self, language: Language) {
        let term = Term::stdout();
        let mnemonic = Mnemonic::from_entropy_in(language, self.0.as_ref()).expect("valid entropy");
        let _ = term.write_line(&mnemonic.to_string());
    }

    pub fn descriptor_recovery_key(&self) -> [u8; 32] {
        let mut hasher = <Sha256 as Digest>::new(); // this is to appease the analyzer
        hasher.update(b"alpen labs strata descriptor recovery file 2024");
        hasher.update(self.0.as_slice());
        hasher.finalize().into()
    }

    pub fn encrypt<R: CryptoRng + RngCore>(
        &self,
        password: &mut Password,
        rng: &mut R,
    ) -> Result<EncryptedSeed, OneOf<(argon2::Error, aes_gcm_siv::Error)>> {
        let mut buf = [0u8; EncryptedSeed::LEN];
        rng.fill_bytes(&mut buf[..PW_SALT_LEN + AES_NONCE_LEN]);

        let seed_encryption_key = password
            .seed_encryption_key(
                &buf[..PW_SALT_LEN].try_into().expect("cannot fail"),
                HashVersion::V0,
            )
            .map_err(OneOf::new)?;

        let (salt_and_nonce, rest) = buf.split_at_mut(PW_SALT_LEN + AES_NONCE_LEN);
        let (seed, _) = rest.split_at_mut(SEED_LEN);
        seed.copy_from_slice(self.0.as_ref());

        let mut cipher = Aes256GcmSiv::new_from_slice(seed_encryption_key.as_ref())
            .expect("should be correct key size");
        let nonce = Nonce::from_slice(&salt_and_nonce[PW_SALT_LEN..]);
        let tag = cipher
            .encrypt_in_place_detached(nonce, &[], seed)
            .map_err(OneOf::new)?;
        buf[(EncryptedSeed::LEN - AES_TAG_LEN)..].copy_from_slice(tag.as_slice());
        Ok(EncryptedSeed(buf))
    }

    pub fn signet_wallet(&self) -> BaseWallet {
        let rootpriv = Xpriv::new_master(Network::Signet, self.0.as_ref()).expect("valid xpriv");
        let base_desc = format!("tr({}/86h/0h/0h", rootpriv);
        let external_desc = format!("{base_desc}/0/*)");
        let internal_desc = format!("{base_desc}/1/*)");
        BaseWallet(
            Wallet::load()
                .descriptor(KeychainKind::External, Some(external_desc.clone()))
                .descriptor(KeychainKind::Internal, Some(internal_desc.clone()))
                .extract_keys(),
            Wallet::create(external_desc, internal_desc),
        )
    }

    pub fn strata_wallet(&self) -> EthereumWallet {
        let l2_private_bytes = {
            let mut hasher = <Sha256 as Digest>::new(); // this is to appease the analyzer
            hasher.update(b"alpen labs strata l2 wallet 2024");
            hasher.update(self.0.as_slice());
            hasher.finalize()
        };

        let signer = PrivateKeySigner::from_field_bytes(&l2_private_bytes).expect("valid slice");

        EthereumWallet::from(signer)
    }
}

pub struct EncryptedSeed([u8; Self::LEN]);

impl EncryptedSeed {
    const LEN: usize = PW_SALT_LEN + AES_NONCE_LEN + SEED_LEN + AES_TAG_LEN;

    fn decrypt(
        mut self,
        password: &mut Password,
    ) -> Result<Seed, OneOf<(argon2::Error, aes_gcm_siv::Error)>> {
        let seed_encryption_key = password
            .seed_encryption_key(
                &self.0[..PW_SALT_LEN].try_into().expect("cannot fail"),
                HashVersion::V0,
            )
            .map_err(OneOf::new)?;

        let mut cipher = Aes256GcmSiv::new_from_slice(seed_encryption_key.as_ref())
            .expect("should be correct key size");
        let (salt_and_nonce, rest) = self.0.split_at_mut(PW_SALT_LEN + AES_NONCE_LEN);
        let (encrypted_seed, tag) = rest.split_at_mut(SEED_LEN);
        let tag = Tag::from_slice(tag);
        let nonce = Nonce::from_slice(&salt_and_nonce[PW_SALT_LEN..]);

        let mut seed = Zeroizing::new([0u8; SEED_LEN]);
        seed.copy_from_slice(encrypted_seed);

        cipher
            .decrypt_in_place_detached(nonce, &[], seed.as_mut(), tag)
            .map_err(OneOf::new)?;

        Ok(Seed(seed))
    }
}

pub fn load_or_create(
    persister: &impl EncryptedSeedPersister,
) -> Result<Seed, OneOf<LoadOrCreateErr>> {
    let term = Term::stdout();
    let _ = term.write_line("Loading encrypted seed...");
    let maybe_encrypted_seed = persister.load().map_err(OneOf::broaden)?;
    if let Some(encrypted_seed) = maybe_encrypted_seed {
        let _ = term.write_line("Opening wallet...");
        let mut password = Password::read(false).map_err(OneOf::new)?;
        match encrypted_seed.decrypt(&mut password) {
            Ok(seed) => {
                let _ = term.write_line("Wallet unlocked");
                Ok(seed)
            }
            Err(e) => {
                let narrowed = e.narrow::<aes_gcm_siv::Error, _>();
                if let Ok(_aes_error) = narrowed {
                    let _ = term.write_line("Incorrect password");
                    return Err(OneOf::new(IncorrectPassword));
                }

                Err(narrowed.unwrap_err().broaden())
            }
        }
    } else {
        let restore = Confirm::new()
            .with_prompt("Do you want to restore a previously created wallet?")
            .interact()
            .map_err(OneOf::new)?;

        let seed = if restore {
            loop {
                let mnemonic: String = Input::new()
                    .with_prompt("Enter your mnemonic")
                    .interact_text()
                    .map_err(OneOf::new)?;

                let mnemonic = match Mnemonic::from_str(&mnemonic) {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = term.write_line(&format!("please try again: {e}"));
                        continue;
                    }
                };
                let entropy = mnemonic.to_entropy();
                if entropy.len() != SEED_LEN {
                    let _ = term.write_line("incorrect entropy length");
                    continue;
                }
                let mut buf = Zeroizing::new([0u8; SEED_LEN]);
                buf.copy_from_slice(&entropy);
                break Seed(buf);
            }
        } else {
            let _ = term.write_line("Creating new wallet");
            Seed::gen(&mut OsRng)
        };

        let mut password = Password::read(true).map_err(OneOf::new)?;
        let password_validation: Result<(), String> = password.validate();
        if let Err(feedback) = password_validation {
            let _ = term.write_line(&format!("Password is weak. {}", feedback));
        };
        let encrypted_seed = match seed.encrypt(&mut password, &mut OsRng) {
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
        Ok(seed)
    }
}

#[cfg(not(target_os = "linux"))]
type PersisterErr = OneOf<(PlatformFailure, NoStorageAccess)>;
#[cfg(target_os = "linux")]
type PersisterErr = OneOf<(io::Error,)>;

#[cfg(target_os = "linux")]
type LoadOrCreateErr = (
    io::Error,
    dialoguer::Error,
    argon2::Error,
    IncorrectPassword,
);

#[cfg(not(target_os = "linux"))]
type LoadOrCreateErr = (
    PlatformFailure,
    NoStorageAccess,
    dialoguer::Error,
    argon2::Error,
    IncorrectPassword,
);

pub trait EncryptedSeedPersister {
    fn save(&self, seed: &EncryptedSeed) -> Result<(), PersisterErr>;
    fn load(&self) -> Result<Option<EncryptedSeed>, PersisterErr>;
    fn delete(&self) -> Result<(), PersisterErr>;
}

#[cfg(target_os = "linux")]
pub use file::*;

#[cfg(target_os = "linux")]
mod file;

#[cfg(not(target_os = "linux"))]
mod keychain;

#[cfg(not(target_os = "linux"))]
pub use keychain::*;

pub mod password;

#[cfg(test)]
mod test {
    use rand::rngs::OsRng;

    use super::*;

    #[test]
    // Test valid seed encryption and decryption
    fn seed_encrypt_decrypt() {
        let mut password = Password::new(String::from("swordfish"));
        let seed = Seed::gen(&mut OsRng);

        let encrypted_seed = seed.encrypt(&mut password, &mut OsRng).unwrap();
        let decrypted_seed = encrypted_seed.decrypt(&mut password).unwrap();

        assert_eq!(seed.0, decrypted_seed.0);
    }

    #[test]
    // Using an evil password fails decryption
    fn evil_password() {
        let mut password = Password::new(String::from("swordfish"));
        let mut evil_password = Password::new(String::from("evil"));
        let seed = Seed::gen(&mut OsRng);

        let encrypted_seed = seed.encrypt(&mut password, &mut OsRng).unwrap();

        assert!(encrypted_seed.decrypt(&mut evil_password).is_err());
    }

    #[test]
    // Using an evil salt fails decryption
    fn evil_salt() {
        let mut password = Password::new(String::from("swordfish"));
        let seed = Seed::gen(&mut OsRng);

        let mut encrypted_seed = seed.encrypt(&mut password, &mut OsRng).unwrap();
        let index = 0;
        encrypted_seed.0[index] = !encrypted_seed.0[index];

        assert!(encrypted_seed.decrypt(&mut password).is_err());
    }

    #[test]
    // Using an evil nonce fails decryption
    fn evil_nonce() {
        let mut password = Password::new(String::from("swordfish"));
        let seed = Seed::gen(&mut OsRng);

        let mut encrypted_seed = seed.encrypt(&mut password, &mut OsRng).unwrap();
        let index = PW_SALT_LEN;
        encrypted_seed.0[index] = !encrypted_seed.0[index];

        assert!(encrypted_seed.decrypt(&mut password).is_err());
    }

    #[test]
    // Using an evil seed fails decryption
    fn evil_seed() {
        let mut password = Password::new(String::from("swordfish"));
        let seed = Seed::gen(&mut OsRng);

        let mut encrypted_seed = seed.encrypt(&mut password, &mut OsRng).unwrap();
        let index = PW_SALT_LEN + AES_NONCE_LEN;
        encrypted_seed.0[index] = !encrypted_seed.0[index];

        assert!(encrypted_seed.decrypt(&mut password).is_err());
    }

    #[test]
    // Using an evil tag fails decryption
    fn evil_tag() {
        let mut password = Password::new(String::from("swordfish"));
        let seed = Seed::gen(&mut OsRng);

        let mut encrypted_seed = seed.encrypt(&mut password, &mut OsRng).unwrap();
        let index = PW_SALT_LEN + AES_NONCE_LEN + SEED_LEN;
        encrypted_seed.0[index] = !encrypted_seed.0[index];

        assert!(encrypted_seed.decrypt(&mut password).is_err());
    }
}
