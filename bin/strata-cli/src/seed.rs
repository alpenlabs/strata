#[cfg(target_os = "linux")]
use std::io::{self, Read};
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
use password::{BadPassword, Password};
use rand::{thread_rng, Rng, RngCore};
use sha2::{Digest, Sha256};
use terrors::OneOf;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const SEED_LEN: usize = 32;
const TAG_LEN: usize = 16;

pub struct BaseWallet(LoadParams, CreateParams);

impl BaseWallet {
    pub fn split(self) -> (LoadParams, CreateParams) {
        (self.0, self.1)
    }
}

#[derive(Clone)]
pub struct Seed([u8; SEED_LEN]);

impl Seed {
    fn gen(rng: &mut impl Rng) -> Self {
        Self(rng.gen())
    }

    pub fn print_mnemonic(&self, language: Language) {
        let term = Term::stdout();
        let mnemonic = Mnemonic::from_entropy_in(language, &self.0).expect("valid entropy");
        let _ = term.write_line(&mnemonic.to_string());
    }

    pub fn descriptor_recovery_key(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"alpen labs strata descriptor recovery file 2024");
        hasher.update(self.0);
        hasher.finalize().into()
    }

    pub fn encrypt(
        &self,
        password: &mut Password,
        rng: &mut impl RngCore,
    ) -> Result<EncryptedSeed, OneOf<(argon2::Error, aes_gcm_siv::Error)>> {
        let mut buf = [0u8; EncryptedSeed::LEN];
        rng.fill_bytes(&mut buf[..SALT_LEN + NONCE_LEN]);

        let seed_encryption_key = password
            .seed_encryption_key(&buf[..SALT_LEN].try_into().expect("cannot fail"))
            .map_err(OneOf::new)?;

        let (salt_and_nonce, rest) = buf.split_at_mut(SALT_LEN + NONCE_LEN);
        let (seed, _) = rest.split_at_mut(SEED_LEN);
        seed.copy_from_slice(&self.0);

        let mut cipher =
            Aes256GcmSiv::new_from_slice(seed_encryption_key).expect("should be correct key size");
        let nonce = Nonce::from_slice(&salt_and_nonce[SALT_LEN..]);
        let tag = cipher
            .encrypt_in_place_detached(nonce, &[], seed)
            .map_err(OneOf::new)?;
        buf[(EncryptedSeed::LEN - TAG_LEN)..].copy_from_slice(tag.as_slice());
        Ok(EncryptedSeed(buf))
    }

    pub fn signet_wallet(&self) -> BaseWallet {
        let rootpriv = Xpriv::new_master(Network::Signet, &self.0).expect("valid xpriv");
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

    pub fn rollup_wallet(&self) -> EthereumWallet {
        let l2_private_bytes = {
            let mut hasher = Sha256::new();
            hasher.update(b"alpen labs strata l2 wallet 2024");
            hasher.update(self.0);
            hasher.finalize()
        };

        let signer = PrivateKeySigner::from_field_bytes(&l2_private_bytes).expect("valid slice");

        EthereumWallet::from(signer)
    }
}

pub struct EncryptedSeed([u8; Self::LEN]);

impl EncryptedSeed {
    const LEN: usize = SALT_LEN + NONCE_LEN + SEED_LEN + TAG_LEN;

    fn decrypt(
        mut self,
        password: &mut Password,
    ) -> Result<Seed, OneOf<(argon2::Error, aes_gcm_siv::Error)>> {
        let seed_encryption_key = password
            .seed_encryption_key(&self.0[..SALT_LEN].try_into().expect("cannot fail"))
            .map_err(OneOf::new)?;

        let mut cipher =
            Aes256GcmSiv::new_from_slice(seed_encryption_key).expect("should be correct key size");
        let (salt_and_nonce, rest) = self.0.split_at_mut(SALT_LEN + NONCE_LEN);
        let (seed, tag) = rest.split_at_mut(SEED_LEN);
        let tag = Tag::from_slice(tag);
        let nonce = Nonce::from_slice(&salt_and_nonce[SALT_LEN..]);

        cipher
            .decrypt_in_place_detached(nonce, &[], seed, tag)
            .map_err(OneOf::new)?;

        Ok(Seed(unsafe { *(seed.as_ptr() as *const [_; SEED_LEN]) }))
    }
}

pub fn load_or_create(
    persister: &impl EncryptedSeedPersister,
) -> Result<Seed, OneOf<LoadOrCreateErr>> {
    let term = Term::stdout();
    let _ = term.write_line("Loading encrypted seed from OS keychain...");
    let maybe_encrypted_seed = persister.load().map_err(|e| OneOf::broaden(e))?;
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
                    let _ = term.write_line("Bad password");
                    return Err(OneOf::new(BadPassword));
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
                let mut buf = [0u8; SEED_LEN];
                buf.copy_from_slice(&entropy);
                break Seed(buf);
            }
        } else {
            let _ = term.write_line("Creating new wallet");
            Seed::gen(&mut thread_rng())
        };

        let mut password = Password::read(true).map_err(OneOf::new)?;
        let encrypted_seed = match seed.encrypt(&mut password, &mut thread_rng()) {
            Ok(es) => es,
            Err(e) => {
                let narrowed = e.narrow::<aes_gcm_siv::Error, _>();
                if let Ok(aes_error) = narrowed {
                    panic!("Failed to encrypt seed: {aes_error:?}");
                }

                return Err(narrowed.unwrap_err().broaden());
            }
        };
        persister
            .save(&encrypted_seed)
            .map_err(|e| OneOf::broaden(e))?;
        Ok(seed)
    }
}

#[cfg(not(target_os = "linux"))]
type PersisterErr = OneOf<(PlatformFailure, NoStorageAccess)>;
#[cfg(target_os = "linux")]
type PersisterErr = OneOf<(io::Error,)>;

#[cfg(target_os = "linux")]
type LoadOrCreateErr = (io::Error, dialoguer::Error, argon2::Error, BadPassword);

#[cfg(not(target_os = "linux"))]
type LoadOrCreateErr = (
    PlatformFailure,
    NoStorageAccess,
    dialoguer::Error,
    argon2::Error,
    BadPassword,
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
