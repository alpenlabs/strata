use std::{io, str::FromStr};

use aes_gcm_siv::{aead::AeadMutInPlace, Aes256GcmSiv, KeyInit, Nonce};
use bdk_wallet::{keys::DescriptorPublicKey, miniscript::Descriptor};
use rand::{thread_rng, Rng};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::{seed::Seed, settings::SETTINGS};

pub struct DescriptorRecovery(File, Aes256GcmSiv);

impl DescriptorRecovery {
    pub async fn add_desc(&mut self, desc: &Descriptor<DescriptorPublicKey>) -> io::Result<()> {
        let mut desc = desc.to_string().as_bytes().to_vec();
        let len = desc.len() as u32;
        let nonce = Nonce::from(thread_rng().gen::<[u8; 12]>());
        self.1
            .encrypt_in_place(&nonce, &len.to_le_bytes(), &mut desc)
            .expect("encryption should succeed");

        self.0.write_all(&len.to_le_bytes()).await?;
        self.0.write_all(nonce.as_slice()).await?;
        self.0.write_all(&desc).await?;
        self.0.sync_all().await?;
        Ok(())
    }

    pub async fn open(seed: &Seed) -> io::Result<Self> {
        let key = seed.descriptor_recovery_key();
        let cipher = Aes256GcmSiv::new(&key.into());
        Ok(Self(File::open(&SETTINGS.descriptor_file).await?, cipher))
    }

    pub async fn read_descs(&mut self) -> io::Result<Vec<Descriptor<DescriptorPublicKey>>> {
        let mut descs = vec![];
        while let Ok(len) = self.0.read_u32_le().await {
            let mut nonce = [0u8; 12];
            self.0.read_exact(&mut nonce).await?;
            let nonce = Nonce::from(nonce);
            let mut buf = Vec::with_capacity(len as usize + 16);
            self.1
                .decrypt_in_place(&nonce, &len.to_le_bytes(), &mut buf)
                .expect("decrypt should succeed");
            let desc = String::from_utf8(buf).expect("desc should be valid utf8");
            descs.push(Descriptor::from_str(&desc).expect("valid descriptor"));
        }
        Ok(descs)
    }
}
