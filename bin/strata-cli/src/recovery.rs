use std::{
    collections::{BTreeMap, HashSet},
    io::{self, Cursor, Read},
    str::FromStr,
    string::FromUtf8Error,
};

use aes_gcm_siv::{aead::AeadMutInPlace, Aes256GcmSiv, KeyInit, Nonce, Tag};
use bdk_wallet::{
    bitcoin::{constants::ChainHash, key::Secp256k1, secp256k1::All, Network},
    keys::{DescriptorPublicKey, DescriptorSecretKey},
    miniscript::{descriptor::DescriptorKeyParseError, Descriptor},
    template::DescriptorTemplateOut,
    wallet_name_from_descriptor,
};
use rand::{thread_rng, Rng};
use terrors::OneOf;
use tokio::io::AsyncReadExt;

use crate::{seed::Seed, settings::SETTINGS};

pub struct DescriptorRecovery(sled::Db, Aes256GcmSiv);

impl DescriptorRecovery {
    pub async fn add_desc(
        &mut self,
        recover_at: u32,
        desc: &DescriptorTemplateOut,
        secp: &Secp256k1<All>,
    ) -> io::Result<()> {
        // the amount of allocation here hurts me emotionally
        // yes, we can't just serialize to bytes 👁️👁️
        let db_key = {
            let mut key = Vec::from(recover_at.to_be_bytes());
            key.extend_from_slice(
                wallet_name_from_descriptor(desc.0.clone(), None, SETTINGS.network, secp)
                    .expect("unique name")
                    .as_bytes(),
            );
            key
        };

        let keymap_iter = desc
            .1
            .iter()
            .map(|(pubk, privk)| [pubk.to_string(), privk.to_string()])
            .map(|[pubk, privk]| {
                (
                    (pubk.as_bytes().len() as u32).to_le_bytes(),
                    pubk,
                    (privk.as_bytes().len() as u32).to_le_bytes(),
                    privk,
                )
            });

        let mut bytes = Vec::new();

        let nonce = Nonce::from(thread_rng().gen::<[u8; 12]>());
        bytes.extend_from_slice(nonce.as_ref());

        let descriptor = desc.0.to_string();
        let desc_bytes = descriptor.as_bytes();

        bytes.extend_from_slice(&(desc_bytes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(desc_bytes);

        let keymap_len = keymap_iter
            .clone()
            .map(|(pubk_len, pubk, privk_len, privk)| {
                pubk_len.len() + pubk.as_bytes().len() + privk_len.len() + privk.as_bytes().len()
            })
            .sum::<usize>();

        bytes.extend_from_slice(&(keymap_len as u64).to_le_bytes());

        for (pubk_len, pubk, privk_len, privk) in keymap_iter {
            bytes.extend_from_slice(&pubk_len);
            bytes.extend_from_slice(&pubk.as_bytes());
            bytes.extend_from_slice(&privk_len);
            bytes.extend_from_slice(&privk.as_bytes());
        }

        let networks = desc
            .2
            .iter()
            .map(|n| n.chain_hash().to_bytes())
            .collect::<Vec<_>>();
        let networks_len = networks.len() as u8;

        bytes.extend_from_slice(&networks_len.to_le_bytes());
        for net in networks {
            bytes.extend_from_slice(&net);
        }

        // nonce (12 bytes) | encrypted_bytes | tag (16 bytes)
        self.1
            .encrypt_in_place(&nonce, &[], &mut bytes)
            .expect("encryption should succeed");

        self.0.insert(db_key, bytes)?;
        self.0.flush_async().await?;
        Ok(())
    }

    pub async fn open(seed: &Seed) -> io::Result<Self> {
        let key = seed.descriptor_recovery_key();
        let cipher = Aes256GcmSiv::new(&key.into());
        Ok(Self(sled::open(&SETTINGS.descriptor_db)?, cipher))
    }

    pub async fn read_descs_after(
        &mut self,
        height: u32,
    ) -> Result<Vec<DescriptorTemplateOut>, OneOf<ReadDescsAfterError>> {
        let mut after_height = self.0.range(height.to_be_bytes()..);
        let mut descs = vec![];
        while let Some(desc_entry) = after_height.next() {
            let mut raw = desc_entry.map_err(OneOf::new)?.1;
            let (nonce, rest) = raw.split_at_mut(12);
            let nonce = Nonce::from_slice(nonce);
            let (encrypted, tag) = rest.split_at_mut(rest.len() - 16);
            let tag = Tag::from_slice(tag);

            self.1
                .decrypt_in_place_detached(&nonce, &[], encrypted, tag)
                .map_err(OneOf::new)?;

            let decrypted = encrypted;
            let mut cursor = Cursor::new(&decrypted);

            let desc_len = cursor.read_u64_le().await.map_err(OneOf::new)? as usize;
            let mut desc_bytes = vec![0u8; desc_len];
            Read::read_exact(&mut cursor, &mut desc_bytes).map_err(OneOf::new)?;
            let desc = String::from_utf8(desc_bytes)
                // oh yeah, nested terrors
                .map_err(|e| OneOf::new(InvalidDescriptor(OneOf::new(e))))?;
            let desc = Descriptor::<DescriptorPublicKey>::from_str(&desc)
                .map_err(|e| OneOf::new(InvalidDescriptor(OneOf::new(e))))?;

            let keymap_len = cursor.read_u64_le().await.map_err(OneOf::new)? as usize;

            let mut to_read = keymap_len;
            let mut keymap = BTreeMap::new();
            while to_read > 0 {
                let pubk_len = cursor.read_u32_le().await.map_err(OneOf::new)? as usize;
                to_read -= 4;

                let mut pubk_bytes = vec![0u8; pubk_len];
                Read::read_exact(&mut cursor, &mut pubk_bytes).map_err(OneOf::new)?;
                to_read -= pubk_len;

                let pubk = String::from_utf8(pubk_bytes)
                    .map_err(|e| OneOf::new(InvalidPublicKey(OneOf::new(e))))?;
                let pubk = DescriptorPublicKey::from_str(&pubk)
                    .map_err(|e| OneOf::new(InvalidPublicKey(OneOf::new(e))))?;

                let privk_len = cursor.read_u32_le().await.map_err(OneOf::new)? as usize;
                to_read -= 4;

                let mut privk_bytes = vec![0u8; privk_len];
                Read::read_exact(&mut cursor, &mut privk_bytes).map_err(OneOf::new)?;
                to_read -= privk_len;

                let privk = String::from_utf8(privk_bytes)
                    .map_err(|e| OneOf::new(InvalidPrivateKey(OneOf::new(e))))?;
                let privk = DescriptorSecretKey::from_str(&privk)
                    .map_err(|e| OneOf::new(InvalidPrivateKey(OneOf::new(e))))?;
                keymap.insert(pubk, privk);
            }

            let networks_len = cursor.read_u8().await.map_err(OneOf::new)?;
            let mut networks = HashSet::with_capacity(networks_len as usize);
            for _ in 0..networks_len {
                let mut chain_hash = [0u8; 32];
                Read::read_exact(&mut cursor, &mut chain_hash).map_err(OneOf::new)?;
                let network = Network::from_chain_hash(ChainHash::from(chain_hash))
                    .ok_or(OneOf::new(InvalidNetwork))?;
                networks.insert(network);
            }

            descs.push((desc, keymap, networks));
        }
        Ok(descs)
    }
}

pub type ReadDescsAfterError = (
    InvalidDescriptor,
    InvalidNetwork,
    InvalidNetworksLen,
    InvalidPrivateKey,
    InvalidPublicKey,
    aes_gcm_siv::Error,
    io::Error,
    sled::Error,
);

#[derive(Debug)]
#[allow(unused)]
pub struct InvalidDescriptor(OneOf<(FromUtf8Error, bdk_wallet::miniscript::Error)>);

#[derive(Debug)]
#[allow(unused)]
pub struct InvalidNetwork;

#[derive(Debug)]
#[allow(unused)]
pub struct InvalidNetworksLen;

#[derive(Debug)]
#[allow(unused)]
pub struct InvalidPrivateKey(OneOf<(FromUtf8Error, DescriptorKeyParseError)>);

#[derive(Debug)]
#[allow(unused)]
pub struct InvalidPublicKey(OneOf<(FromUtf8Error, DescriptorKeyParseError)>);
