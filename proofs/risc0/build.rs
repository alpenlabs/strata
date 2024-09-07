fn main() {
    std::env::set_var("CC", "clang");
    std::env::set_var(
        "CFLAGS_riscv32im_risc0_zkvm_elf",
        "-target riscv32-unknown-elf",
    );

    if cfg!(feature = "prover")
        && std::env::var("SKIP_GUEST_BUILD").is_err()
        && std::env::var("CARGO_CFG_CLIPPY").is_err()
    {
        risc0_build::embed_methods();
    } else {
        // Return mock ELF
        let out_dir = std::env::var_os("OUT_DIR").unwrap();
        let out_dir = std::path::Path::new(&out_dir);
        let methods_path = out_dir.join("methods.rs");

        let elf = r#"
            pub const RETH_RISC0_ELF: &[u8] = &[];
            pub const CL_BLOCK_STF_ELF: &[u8] = &[];
            pub const GUEST_CL_AGG_ELF: &[u8] = &[];
        "#;

        std::fs::write(methods_path, elf).expect("Failed to write mock rollup elf");
    }
}
