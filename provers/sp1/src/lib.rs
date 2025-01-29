use std::path::PathBuf;

use anyhow::{Context, Result};

include!(concat!(env!("OUT_DIR"), "/methods.rs"));

pub fn export_elf(elf_path: &PathBuf) -> Result<()> {
    // Create output directory if it doesn't exist
    fs::create_dir_all(elf_path).context("Failed to create ELF output directory")?;

    // Get the current crate's directory
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Iterate over entries in cargo_dir
    for entry in fs::read_dir(&cargo_dir)
        .with_context(|| format!("Failed to read directory {:?}", cargo_dir))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                if folder_name.starts_with("guest-") {
                    let cache_dir = path.join("cache");
                    if cache_dir.is_dir() {
                        for file in fs::read_dir(&cache_dir).with_context(|| {
                            format!("Failed to read cache directory {:?}", cache_dir)
                        })? {
                            let file = file?;
                            let file_path = file.path();
                            if file_path.is_file()
                                && file_path.extension().and_then(|s| s.to_str()) == Some("elf")
                            {
                                let file_name = file_path.file_name().unwrap();
                                let dest_file = elf_path.join(file_name);
                                fs::copy(&file_path, &dest_file).with_context(|| {
                                    format!("Failed to copy {:?} to {:?}", file_path, dest_file)
                                })?;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
