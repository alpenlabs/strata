use std::str::FromStr;

use aes_gcm_siv::{aead::AeadMutInPlace, Aes256GcmSiv, KeyInit, Nonce, Tag};
use argon2::Argon2;
use bip39::{Language, Mnemonic};
use console::Term;
use dialoguer::{Confirm, Input, Password as InputPassword};
use keyring::{Credential, Entry, Error};
use rand::{thread_rng, Rng, RngCore};
use terrors::OneOf;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const SEED_LEN: usize = 32;
const TAG_LEN: usize = 16;

pub struct Seed([u8; SEED_LEN]);

impl Seed {
    pub fn gen(rng: &mut impl Rng) -> Self {
        Self(rng.gen())
    }

    pub fn print_mnemonic(&self, language: Language) {
        let term = Term::stdout();
        let mnemonic = Mnemonic::from_entropy_in(language, &self.0).expect("valid entropy");
        let _ = term.write_line(&mnemonic.to_string());
    }

    fn encrypt(
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

    pub fn load_or_create() -> Result<
        Seed,
        OneOf<(
            PlatformFailure,
            NoStorageAccess,
            dialoguer::Error,
            argon2::Error,
            BadPassword,
        )>,
    > {
        let maybe_encrypted_seed = EncryptedSeed::load().map_err(OneOf::broaden)?;
        let term = Term::stdout();
        if let Some(encrypted_seed) = maybe_encrypted_seed {
            let _ = term.write_line("Opening wallet");
            let mut password = Password::read(false).map_err(OneOf::new)?;
            match encrypted_seed.decrypt(&mut password) {
                Ok(seed) => {
                    let _ = term.write_line("Wallet is open");
                    return Ok(seed);
                }
                Err(e) => {
                    let narrowed = e.narrow::<aes_gcm_siv::Error, _>();
                    if let Ok(_aes_error) = narrowed {
                        let _ = term.write_line("Bad password");
                        return Err(OneOf::new(BadPassword));
                    }

                    return Err(narrowed.unwrap_err().broaden());
                }
            }
        } else {
            let restore = Confirm::new()
                .with_prompt("Do you want to restore a previously created wallet?")
                .interact()
                .map_err(OneOf::new)?;

            let seed = if restore {
                let mnemonic: String = Input::new()
                    .with_prompt("Enter your mnemonic")
                    .interact_text()
                    .map_err(OneOf::new)?;

                loop {
                    let mnemonic = match Mnemonic::from_str(&mnemonic) {
                        Ok(m) => m,
                        Err(e) => {
                            let _ = term.write_line(&format!("please try again: {e}"));
                            continue;
                        }
                    };
                    let entropy = mnemonic.to_entropy();
                    if entropy.len() != SEED_LEN {
                        let _ = term.write_line(&format!("incorrect entropy length"));
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
            encrypted_seed.save().map_err(OneOf::broaden)?;
            Ok(seed)
        }
    }
}

pub struct Password {
    inner: String,
    seed_encryption_key: Option<[u8; 32]>,
}

impl Password {
    fn read(new: bool) -> Result<Self, dialoguer::Error> {
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

    fn seed_encryption_key(&mut self, salt: &[u8; SALT_LEN]) -> Result<&[u8; 32], argon2::Error> {
        match self.seed_encryption_key {
            Some(ref key) => Ok(&key),
            None => {
                let mut sek = [0u8; 32];
                Argon2::default().hash_password_into(self.inner.as_bytes(), salt, &mut sek)?;
                self.seed_encryption_key = Some(sek);
                self.seed_encryption_key(salt)
            }
        }
    }
}

struct EncryptedSeed([u8; Self::LEN]);

pub fn reset() -> Result<
    (),
    OneOf<(
        PlatformFailure,
        NoStorageAccess,
        Ambiguous,
        dialoguer::Error,
        argon2::Error,
    )>,
> {
    let term = Term::stdout();

    if let Some(seed) = EncryptedSeed::load().map_err(OneOf::broaden)? {
        let mut password = Password::read(false).map_err(OneOf::new)?;
        if let Err(e) = seed.decrypt(&mut password) {
            let narrowed = e.narrow::<aes_gcm_siv::Error, _>();
            if let Ok(_aes_error) = narrowed {
                let _ = term.write_line("Bad password");
                std::process::exit(1);
            }

            Err(narrowed.unwrap_err().broaden())?;
        }
    } else {
        let _ = term.write_line("Seed is already empty");
        return Ok(());
    }

    let _ = term.write_line("Wiping seed");
    let entry = EncryptedSeed::entry().map_err(OneOf::broaden)?;
    if let Err(e) = entry.delete_credential().map_err(keyring_oneof) {
        // if e is NOT a NoEntry error
        if let Err(e) = e.narrow::<NoEntry, _>() {
            match e.narrow::<Ambiguous, _>() {
                // e is Ambiguous
                Ok(e) => Err(OneOf::new(e))?,
                // e is something else. We shouldn't be here.
                Err(e) => panic!("bad error: {e:?}"),
            }
        }
    }
    let _ = term.write_line("Seed has been wiped from OS keychain");
    Ok(())
}

impl EncryptedSeed {
    const LEN: usize = SALT_LEN + NONCE_LEN + SEED_LEN + TAG_LEN;

    fn entry() -> Result<Entry, OneOf<(PlatformFailure, NoStorageAccess)>> {
        Ok(Entry::new("strata", "default")
            .map_err(keyring_oneof)
            .map_err(|e| {
                e.subset()
                    .unwrap_or_else(|e| panic!("errored subsetting keychain error: {e:?}"))
            })?)
    }

    fn save(&self) -> Result<(), OneOf<(PlatformFailure, NoStorageAccess)>> {
        let entry = Self::entry()?;
        entry
            .set_secret(&self.0)
            .map_err(keyring_oneof)
            .map_err(|e| {
                e.subset()
                    .unwrap_or_else(|e| panic!("errored subsetting keychain error: {e:?}"))
            })
    }

    fn load() -> Result<Option<Self>, OneOf<(PlatformFailure, NoStorageAccess)>> {
        let entry = Self::entry()?;

        let secret = match entry.get_secret().map_err(keyring_oneof) {
            Ok(s) => s,
            Err(e) => {
                let no_entry = e.narrow::<NoEntry, _>();
                if no_entry.is_ok() {
                    return Ok(None);
                }

                let bad_encoding = no_entry.unwrap_err().narrow::<BadEncoding, _>();
                if bad_encoding.is_ok() {
                    let _ = entry.delete_credential();
                    return Ok(None);
                }

                return Err(bad_encoding
                    .unwrap_err()
                    .subset()
                    .unwrap_or_else(|e| panic!("errored subsetting keychain error: {e:?}")));
            }
        };

        if secret.len() == Self::LEN {
            Ok(Some(Self(secret.try_into().unwrap())))
        } else {
            let _ = entry.delete_credential();
            Ok(None)
        }
    }

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
            .decrypt_in_place_detached(&nonce, &[], seed, tag)
            .map_err(OneOf::new)?;

        Ok(Seed(unsafe { *(seed.as_ptr() as *const [_; SEED_LEN]) }))
    }
}

// below is wrapper around [`keyring::Error`] so it can be used with OneOf
// to more precisely handle errors

#[derive(Debug)]
pub struct BadPassword;

/// This indicates runtime failure in the underlying
/// platform storage system.  The details of the failure can
/// be retrieved from the attached platform error.
#[derive(Debug)]
pub struct PlatformFailure(Box<dyn std::error::Error + Send + Sync>);
/// This indicates that the underlying secure storage
/// holding saved items could not be accessed.  Typically this
/// is because of access rules in the platform; for example, it
/// might be that the credential store is locked.  The underlying
/// platform error will typically give the reason.
#[derive(Debug)]
pub struct NoStorageAccess(Box<dyn std::error::Error + Send + Sync>);
/// This indicates that there is no underlying credential
/// entry in the platform for this entry.  Either one was
/// never set, or it was deleted.
#[derive(Debug)]
pub struct NoEntry;
/// This indicates that the retrieved password blob was not
/// a UTF-8 string.  The underlying bytes are available
/// for examination in the attached value.
#[derive(Debug)]
pub struct BadEncoding(Vec<u8>);
/// This indicates that one of the entry's credential
/// attributes exceeded a
/// length limit in the underlying platform.  The
/// attached values give the name of the attribute and
/// the platform length limit that was exceeded.
#[derive(Debug)]
pub struct TooLong {
    name: String,
    limit: u32,
}
/// This indicates that one of the entry's required credential
/// attributes was invalid.  The
/// attached value gives the name of the attribute
/// and the reason it's invalid.
#[derive(Debug)]
pub struct Invalid {
    name: String,
    reason: String,
}
/// This indicates that there is more than one credential found in the store
/// that matches the entry.  Its value is a vector of the matching credentials.
#[derive(Debug)]
pub struct Ambiguous(Vec<Box<Credential>>);

type KeyRingErrors = (
    PlatformFailure,
    NoStorageAccess,
    NoEntry,
    BadEncoding,
    TooLong,
    Invalid,
    Ambiguous,
);

fn keyring_oneof(err: keyring::Error) -> OneOf<KeyRingErrors> {
    match err {
        Error::PlatformFailure(error) => OneOf::new(PlatformFailure(error)),
        Error::NoStorageAccess(error) => OneOf::new(NoStorageAccess(error)),
        Error::NoEntry => OneOf::new(NoEntry),
        Error::BadEncoding(vec) => OneOf::new(BadEncoding(vec)),
        Error::TooLong(name, limit) => OneOf::new(TooLong { name, limit }),
        Error::Invalid(name, reason) => OneOf::new(Invalid { name, reason }),
        Error::Ambiguous(vec) => OneOf::new(Ambiguous(vec)),
        _ => todo!(),
    }
}
