use std::{
    env, fs,
    path::{Path, PathBuf},
};

use sp1_helper::build_program;

const RISC_V_COMPILER: &str = "/opt/riscv/bin/riscv-none-elf-gcc";
// const ELF_FILE_PATH: &str = "guest-reth-stf/elf/riscv32im-succinct-zkvm-elf";
const ELF_FILE_PATH: &str = "guest-cl-stf/elf/riscv32im-succinct-zkvm-elf";
const MOCK_ELF_CONTENT: &str = r#"
    pub const RETH_SP1_ELF: &[u8] = &[];
    pub const CL_BLOCK_STF_ELF: &[u8] = &[];
"#;

fn main() {
    let out_dir = get_output_dir();
    let methods_path = out_dir.join("methods.rs");

    if cfg!(feature = "prover")
        && std::env::var("SKIP_GUEST_BUILD").is_err()
        && std::env::var("CARGO_CFG_CLIPPY").is_err()
    {
        setup_compiler();
        build_program("guest-cl-stf");
        let elf_content = generate_elf_content(ELF_FILE_PATH);
        fs::write(&methods_path, elf_content).expect("failed writing to methods path");
        // fs::write(methods_path, MOCK_ELF_CONTENT).expect("failed writing to methods path");
    } else {
        fs::write(methods_path, MOCK_ELF_CONTENT).expect("failed writing to methods path");
    }
}

fn get_output_dir() -> PathBuf {
    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR environment variable not set");
    Path::new(&out_dir).to_path_buf()
}

fn setup_compiler() {
    env::set_var("CC_riscv32im_succinct_zkvm_elf", RISC_V_COMPILER);
}

fn generate_elf_content(elf_path: &str) -> String {
    let contents = fs::read(elf_path).expect("failed to find sp1 elf");
    let contents_str = contents
        .iter()
        .map(|byte| format!("{:#04x}", byte))
        .collect::<Vec<String>>()
        .join(", ");

    format!(
        r#"
        pub const CL_BLOCK_STF_ELF: &[u8] = &[{}];
    "#,
        contents_str
    )
}
