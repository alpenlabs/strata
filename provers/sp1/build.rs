use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self},
    path::{Path, PathBuf},
};

use bincode::{deserialize, serialize};
use sha2::{Digest, Sha256};
use sp1_helper::{build_program_with_args, BuildArgs};
use sp1_sdk::{HashableKey, MockProver, Prover, SP1VerifyingKey};

// Path to the RISC-V compiler
const RISC_V_COMPILER: &str = "/opt/riscv/bin/riscv-none-elf-gcc";

// Guest program names
const EVM_EE_STF: &str = "guest-evm-ee-stf";
const CL_STF: &str = "guest-cl-stf";
const BTC_BLOCKSPACE: &str = "guest-btc-blockspace";
const L1_BATCH: &str = "guest-l1-batch";
const CL_AGG: &str = "guest-cl-agg";
const CHECKPOINT: &str = "guest-checkpoint";

/// Returns a map of program dependencies.
fn get_program_dependencies() -> HashMap<&'static str, Vec<&'static str>> {
    let mut dependencies = HashMap::new();
    dependencies.insert(L1_BATCH, vec![BTC_BLOCKSPACE]);
    dependencies.insert(CL_STF, vec![EVM_EE_STF]);
    dependencies.insert(CL_AGG, vec![CL_STF]);
    dependencies.insert(CHECKPOINT, vec![L1_BATCH, CL_AGG]);
    dependencies
}

fn main() {
    // List of guest programs to build
    let guest_programs = [
        BTC_BLOCKSPACE,
        L1_BATCH,
        EVM_EE_STF,
        CL_STF,
        CL_AGG,
        CHECKPOINT,
    ];

    // String to accumulate the contents of methods.rs file
    let mut methods_file_content = String::new();

    // HashSet to keep track of programs that have been built
    let mut built_programs = HashSet::new();

    // Get the dependencies between programs
    let dependencies = get_program_dependencies();

    // HashMap to store results: mapping from elf_name to (elf_contents, vk_hash_u32, vk_hash_str)
    let mut results = HashMap::new();

    // HashMap to store vk hashes of programs
    let mut vk_hashes = HashMap::new();

    // Build each guest program along with its dependencies
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
    for (elf_name, (elf_contents, vk_hash_u32, vk_hash_str)) in &results {
        let elf_name_id = format!("{}_ID", elf_name);
        let elf_name_str = format!("{}_STR", elf_name);
        methods_file_content.push_str(&format!(
            r#"
pub const {}: &[u8] = &{:?};
pub const {}: &[u32] = &{:?};
pub const {}: &str = "{}";
"#,
            elf_name, elf_contents, elf_name_id, vk_hash_u32, elf_name_str, vk_hash_str
        ));
    }

    // Write the accumulated methods_file_content to methods.rs in the output directory
    let out_dir = get_output_dir();
    let methods_path = out_dir.join("methods.rs");
    fs::write(&methods_path, methods_file_content).unwrap_or_else(|e| {
        panic!(
            "Failed to write methods.rs file at {}: {}",
            methods_path.display(),
            e
        )
    });
}

/// Recursively builds the given program along with its dependencies.
fn build_program_with_dependencies(
    program: &str,
    dependencies: &HashMap<&str, Vec<&str>>,
    built_programs: &mut HashSet<String>,
    results: &mut HashMap<String, (Vec<u8>, [u32; 8], String)>,
    vk_hashes: &mut HashMap<String, [u32; 8]>,
) {
    // If the program has already been built, return early
    if built_programs.contains(program) {
        return;
    }

    // Build dependencies first
    if let Some(deps) = dependencies.get(program) {
        for dep in deps {
            build_program_with_dependencies(dep, dependencies, built_programs, results, vk_hashes);
        }

        // After dependencies are built, write vks.rs for the current program
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
            let vks_path = Path::new(program).join("src").join("vks.rs");
            fs::write(&vks_path, vks_content)
                .unwrap_or_else(|e| panic!("Failed to write vks.rs for {}: {}", program, e));
        }
    }

    // Now build the current program
    let elf_name = format!("{}_ELF", program.to_uppercase().replace("-", "_"));

    // Build the program and generate ELF contents and VK hash
    let (elf_contents, vk_hash_u32, vk_hash_str) = generate_elf_contents_and_vk_hash(program);

    // Store the results
    results.insert(
        elf_name.clone(),
        (elf_contents.clone(), vk_hash_u32, vk_hash_str),
    );
    vk_hashes.insert(program.to_string(), vk_hash_u32);
    built_programs.insert(program.to_string());
}

/// Returns the output directory for the build artifacts.
fn get_output_dir() -> PathBuf {
    env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .expect("OUT_DIR environment variable is not set. Cannot determine output directory.")
}

/// Checks if the cache is valid by comparing the expected ID with the saved ID.
fn is_cache_valid(expected_id: &[u8; 32], paths: &[PathBuf; 4]) -> bool {
    // Check if any required files are missing
    if paths.iter().any(|path| !path.exists()) {
        return false;
    }

    // Attempt to read the saved ID
    let saved_id = match fs::read(&paths[1]) {
        Ok(data) => data,
        Err(_) => return false,
    };

    expected_id == saved_id.as_slice()
}

/// Ensures the cache is valid and returns the ELF contents and SP1 Verifying Key.
fn ensure_cache_validity(program: &str) -> Result<(Vec<u8>, SP1VerifyingKey), String> {
    let cache_dir = format!("{}/cache", program);
    let paths = ["elf", "id", "vk", "pk"]
        .map(|file| Path::new(&cache_dir).join(format!("{}.{}", program, file)));

    // Attempt to read the ELF file
    let elf = fs::read(&paths[0])
        .map_err(|e| format!("Failed to read ELF file {}: {}", paths[0].display(), e))?;
    let elf_hash: [u8; 32] = Sha256::digest(&elf).into();

    if !is_cache_valid(&elf_hash, &paths) {
        // Cache is invalid, need to generate vk and pk
        let client = MockProver::new();
        let (pk, vk) = client.setup(&elf);

        fs::write(&paths[1], elf_hash)
            .map_err(|e| format!("Failed to write ID file {}: {}", paths[1].display(), e))?;

        fs::write(&paths[2], serialize(&vk).expect("VK serialization failed"))
            .map_err(|e| format!("Failed to write VK file {}: {}", paths[2].display(), e))?;

        fs::write(&paths[3], serialize(&pk).expect("PK serialization failed"))
            .map_err(|e| format!("Failed to write PK file {}: {}", paths[3].display(), e))?;

        Ok((elf, vk))
    } else {
        // Cache is valid, read the VK
        let serialized_vk = fs::read(&paths[2])
            .map_err(|e| format!("Failed to read VK file {}: {}", paths[2].display(), e))?;
        let vk: SP1VerifyingKey =
            deserialize(&serialized_vk).map_err(|e| format!("VK deserialization failed: {}", e))?;
        Ok((elf, vk))
    }
}

/// Generates the ELF contents and VK hash for a given program.
fn generate_elf_contents_and_vk_hash(program: &str) -> (Vec<u8>, [u32; 8], String) {
    // Setup compiler
    env::set_var("CC_riscv32im_succinct_zkvm_elf", RISC_V_COMPILER);

    let build_args = BuildArgs {
        elf_name: format!("{}.elf", program),
        output_directory: "cache".to_owned(),
        ..Default::default()
    };

    // Build the program
    build_program_with_args(program, build_args);

    // Now, ensure cache validity
    let (elf, vk) = ensure_cache_validity(program)
        .expect("Failed to ensure cache validity after building program");
    (elf, vk.hash_u32(), vk.bytes32())
}
