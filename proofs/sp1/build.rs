use std::{
    env, fs,
    path::{Path, PathBuf},
};

use sp1_helper::build_program;

const RISC_V_COMPILER: &str = "/opt/riscv/bin/riscv-none-elf-gcc";
const PROGRAMS_TO_BUILD: [&str; 2] = ["guest-reth-stf", "btc-blockspace"];

fn main() {
    let guest_programs = get_guest_programs();
    let mut methods_file_content = String::new();

    for program in guest_programs {
        let elf_name = format!("{}-ELF", program.to_uppercase()).replace("-", "_");
        let mut elf_content = generate_mock_elf_content(&elf_name);

        if is_build_enabled() && PROGRAMS_TO_BUILD.contains(&program.as_str()) {
            setup_compiler();
            build_program(&program);

            let elf_path = format!("{}/elf/riscv32im-succinct-zkvm-elf", program);
            elf_content = generate_elf_content(&elf_name, &elf_path);
        }

        methods_file_content += &elf_content;
    }

    let out_dir = get_output_dir();
    let methods_path = out_dir.join("methods.rs");
    fs::write(&methods_path, methods_file_content).expect("failed writing to methods path");
}

fn is_build_enabled() -> bool {
    cfg!(feature = "prover")
        && std::env::var("SKIP_GUEST_BUILD").is_err()
        && std::env::var("CARGO_CFG_CLIPPY").is_err()
}

fn get_output_dir() -> PathBuf {
    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR environment variable not set");
    Path::new(&out_dir).to_path_buf()
}

fn setup_compiler() {
    env::set_var("CC_riscv32im_succinct_zkvm_elf", RISC_V_COMPILER);
}

fn generate_elf_content(elf_name: &str, elf_path: &str) -> String {
    let contents = fs::read(elf_path).expect("failed to find sp1 elf");
    let contents_str = contents
        .iter()
        .map(|byte| format!("{:#04x}", byte))
        .collect::<Vec<String>>()
        .join(", ");

    format!(
        r#"
        pub const {}: &[u8] = &[{}];
    "#,
        elf_name, contents_str
    )
}

fn generate_mock_elf_content(elf_name: &str) -> String {
    format!(
        r#"
            pub const {}:&[u8] = &[];
        "#,
        elf_name
    )
}

fn get_guest_programs() -> Vec<String> {
    let path = Path::new(".");
    let prefix = "guest-";

    fs::read_dir(path)
        .expect("Unable to read directory")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .filter_map(|entry| {
            entry
                .file_name()
                .into_string()
                .ok()
                .filter(|name| name.starts_with(prefix))
        })
        .collect::<Vec<String>>()
}
