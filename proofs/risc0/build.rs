fn main() {
    if cfg!(feature = "prover") {
        risc0_build::embed_methods();
    } else {
        // Return mock ELF
        let out_dir = std::env::var_os("OUT_DIR").unwrap();
        let out_dir = std::path::Path::new(&out_dir);
        let methods_path = out_dir.join("methods.rs");

        let elf = r#"
            pub const GUEST_RETH_STF_ELF: &[u8] = &[];
            pub const GUEST_RETH_STF_ID: [u32; 8] = [0u32;8];
        "#;

        std::fs::write(methods_path, elf).expect("Failed to write mock rollup elf");
    }
}
