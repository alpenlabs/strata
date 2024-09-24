#[derive(Clone, Debug)]
pub struct FilePersister {
    file: PathBuf,
}

impl EncryptedSeedPersister for FilePersister {
    fn save(&self, seed: &EncryptedSeed) -> Result<(), PersisterErr> {
        let mut file = File::options().create(true).write(true).open(&self.file)?;
        file.write_all(&seed.0)?;
        file.sync_all()?;
        Ok(())
    }

    fn load(&self) -> Result<Option<EncryptedSeed>, PersisterErr> {
        let mut file = match File::options().read(true).open(&self.file) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut buf = [0u8; EncryptedSeed::LEN];
        let bytes_read = file.read(&mut buf)?;
        if bytes_read != buf.len() {
            self.delete()?;
            return Ok(None);
        }
        Ok(Some(EncryptedSeed(buf)))
    }

    fn delete(&self) -> Result<(), PersisterErr> {
        remove_file(&self.file)
    }
}
