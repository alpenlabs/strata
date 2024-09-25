use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

#[cfg(not(debug_assertions))]
use sp1_helper::build_program;
#[cfg(not(debug_assertions))]
use sp1_sdk::{HashableKey, MockProver, Prover};
#[cfg(not(debug_assertions))]
const RISC_V_COMPILER: &str = "/opt/riscv/bin/riscv-none-elf-gcc";

const EVM_EE_STF: &str = "guest-evm-ee-stf";
const CL_STF: &str = "guest-cl-stf";
const BTC_BLOCKSPACE: &str = "guest-btc-blockspace";
const L1_BATCH: &str = "guest-l1-batch";

#[cfg(not(debug_assertions))]
const PROGRAMS_TO_BUILD: &[&str] = &[EVM_EE_STF, CL_STF, BTC_BLOCKSPACE, L1_BATCH];

fn get_program_dependencies() -> HashMap<&'static str, Vec<&'static str>> {
    let mut dependencies = HashMap::new();
    dependencies.insert(L1_BATCH, vec![BTC_BLOCKSPACE]);
    dependencies.insert(CL_STF, vec![EVM_EE_STF]);
    dependencies
}

fn main() {
    let guest_programs = get_guest_programs();
    let mut methods_file_content = String::new();
    let mut built_programs = HashSet::new();
    let dependencies = get_program_dependencies();
    let mut results = HashMap::new();
    let mut vk_hashes = HashMap::new();

    for program in &guest_programs {
        build_program_with_dependencies(
            program,
            &dependencies,
            &mut built_programs,
            &mut results,
            &mut vk_hashes,
        );
    }

    // Write the methods.rs file with ELF contents and VK hashes
    for (elf_name, (elf_contents, vk_hash_u32)) in &results {
        let elf_name_id = format!("{}_ID", elf_name);
        let contents_str = elf_contents
            .iter()
            .map(|byte| format!("{:#04x}", byte))
            .collect::<Vec<_>>()
            .join(", ");

        methods_file_content.push_str(&format!(
            r#"
pub const {}: &[u8] = &[{}];
pub const {}: &[u32] = &{:?};
"#,
            elf_name, contents_str, elf_name_id, vk_hash_u32
        ));
    }

    let out_dir = get_output_dir();
    let methods_path = out_dir.join("methods.rs");
    fs::write(&methods_path, methods_file_content).expect("Failed writing to methods path");
}

fn build_program_with_dependencies(
    program: &str,
    dependencies: &HashMap<&str, Vec<&str>>,
    built_programs: &mut HashSet<String>,
    results: &mut HashMap<String, (Vec<u8>, [u32; 8])>,
    vk_hashes: &mut HashMap<String, [u32; 8]>,
) {
    if built_programs.contains(program) {
        return;
    }

    // Build dependencies first
    if let Some(deps) = dependencies.get(program) {
        for dep in deps {
            build_program_with_dependencies(dep, dependencies, built_programs, results, vk_hashes);
        }

        // After dependencies are built, write vks.rs for current program
        let mut vks_content = String::new();
        for dep in deps {
            if let Some(vk_hash) = vk_hashes.get(*dep) {
                let elf_name = format!("{}_ELF", dep.to_uppercase().replace("-", "_"));
                let elf_name_id = format!("{}_ID", elf_name);
                vks_content.push_str(&format!(
                    "pub const {}: &[u32; 8] = &{:?};\n",
                    elf_name_id, vk_hash
                ));
            }
        }

        // Only write vks.rs if there are dependencies
        if !vks_content.is_empty() {
            // Write vks.rs in the program's directory before building it
            let vks_path = Path::new(program).join("src").join("vks.rs");
            fs::write(&vks_path, vks_content).expect("Failed writing to vks.rs");
        }
    }

    // Now build the current program
    let elf_name = format!("{}_ELF", program.to_uppercase().replace("-", "_"));
    let (elf_contents, vk_hash_u32) = generate_elf_contents_and_vk_hash(program);

    results.insert(elf_name.clone(), (elf_contents.clone(), vk_hash_u32));
    vk_hashes.insert(program.to_string(), vk_hash_u32);
    built_programs.insert(program.to_string());
}

// fn is_build_enabled() -> bool {
//     env::var("SKIP_GUEST_BUILD").is_err() && env::var("CARGO_CFG_CLIPPY").is_err()
// }

fn get_output_dir() -> PathBuf {
    env::var_os("OUT_DIR")
        .expect("OUT_DIR environment variable not set")
        .into()
}

#[cfg(not(debug_assertions))]
fn generate_elf_contents_and_vk_hash(program: &str) -> (Vec<u8>, [u32; 8]) {
    // Setup compiler
    env::set_var("CC_riscv32im_succinct_zkvm_elf", RISC_V_COMPILER);

    // Ensure the vks.rs is in place before building the program
    build_program(program);

    let elf_path = format!("{}/elf/riscv32im-succinct-zkvm-elf", program);

    let contents = fs::read(elf_path).expect("Failed to find SP1 ELF");
    let client = MockProver::new();
    let (_pk, vk) = client.setup(&contents);
    (contents, vk.hash_u32())
}

#[cfg(debug_assertions)]
fn generate_elf_contents_and_vk_hash(_program: &str) -> (Vec<u8>, [u32; 8]) {
    (Vec::new(), [0u32; 8])
}

fn get_guest_programs() -> Vec<String> {
    let prefix = "guest-";
    fs::read_dir(".")
        .expect("Unable to read current directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                let name = entry.file_name().into_string().ok()?;
                if name.starts_with(prefix) {
                    Some(name)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}
