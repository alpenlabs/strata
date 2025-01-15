use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self},
    path::{Path, PathBuf},
};

#[cfg(not(debug_assertions))]
use bincode::{deserialize, serialize};
#[cfg(not(debug_assertions))]
use cargo_metadata::MetadataCommand;
#[cfg(not(debug_assertions))]
use sha2::{Digest, Sha256};
#[cfg(not(debug_assertions))]
use sp1_helper::{build_program_with_args, BuildArgs};
#[cfg(not(debug_assertions))]
use sp1_sdk::{HashableKey, ProverClient, SP1VerifyingKey};

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

    // String to accumulate the contents of methods.rs file
    // Start with the necessary use statements
    let mut methods_file_content = String::from(
        r#"
use once_cell::sync::Lazy;
use std::fs;
"#,
    );

    // Write the methods.rs file with ELF contents and VK hashes
    for (program_name, (vk_hash_u32, vk_hash_str)) in &results {
        let program_name_upper = program_name.to_uppercase().replace("-", "_");
        let base_path = Path::new(program_name)
            .canonicalize()
            .expect("Cache directory not found");
        let base_path_str = base_path
            .to_str()
            .expect("Failed to convert path to string");

        let full_path_str = format!("{}/cache/{}", base_path_str, program_name);
        methods_file_content.push_str(&format!(
            r#"
pub static {0}_ELF: Lazy<Vec<u8>> = Lazy::new(||{{ fs::read("{1}.elf").expect("Cannot find ELF") }});
pub static {0}_PK: Lazy<Vec<u8>> = Lazy::new(||{{ fs::read("{1}.pk").expect("Cannot find PK") }});
pub static {0}_VK: Lazy<Vec<u8>> = Lazy::new(||{{ fs::read("{1}.vk").expect("Cannot find VK") }});
pub const {0}_VK_HASH_U32: &[u32] = &{2:?};
pub const {0}_VK_HASH_STR: &str = "{3}";
"#,
            program_name_upper, full_path_str, vk_hash_u32, vk_hash_str
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
    results: &mut HashMap<String, ([u32; 8], String)>,
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

    // Build the program and generate ELF contents and VK hash
    let (vk_hash_u32, vk_hash_str) = generate_elf_contents_and_vk_hash(program);

    // Store the results
    results.insert(program.to_string(), (vk_hash_u32, vk_hash_str));
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
#[cfg(not(debug_assertions))]
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
#[cfg(not(debug_assertions))]
fn ensure_cache_validity(program: &str) -> Result<SP1VerifyingKey, String> {
    let cache_dir = format!("{}/cache", program);
    let paths = ["elf", "id", "vk", "pk"]
        .map(|file| Path::new(&cache_dir).join(format!("{}.{}", program, file)));

    // Attempt to read the ELF file
    let elf = fs::read(&paths[0])
        .map_err(|e| format!("Failed to read ELF file {}: {}", paths[0].display(), e))?;
    let elf_hash: [u8; 32] = Sha256::digest(&elf).into();

    if !is_cache_valid(&elf_hash, &paths) {
        // Cache is invalid, need to generate vk and pk
        let client = ProverClient::from_env();
        let (pk, vk) = client.setup(&elf);

        fs::write(&paths[1], elf_hash)
            .map_err(|e| format!("Failed to write ID file {}: {}", paths[1].display(), e))?;

        fs::write(&paths[2], serialize(&vk).expect("VK serialization failed"))
            .map_err(|e| format!("Failed to write VK file {}: {}", paths[2].display(), e))?;

        fs::write(&paths[3], serialize(&pk).expect("PK serialization failed"))
            .map_err(|e| format!("Failed to write PK file {}: {}", paths[3].display(), e))?;

        Ok(vk)
    } else {
        // Cache is valid, read the VK
        let serialized_vk = fs::read(&paths[2])
            .map_err(|e| format!("Failed to read VK file {}: {}", paths[2].display(), e))?;
        let vk: SP1VerifyingKey =
            deserialize(&serialized_vk).map_err(|e| format!("VK deserialization failed: {}", e))?;
        Ok(vk)
    }
}

/// Generates the ELF contents and VK hash for a given program.
#[cfg(not(debug_assertions))]
fn generate_elf_contents_and_vk_hash(program: &str) -> ([u32; 8], String) {
    let features = {
        #[cfg(feature = "mock")]
        {
            vec!["mock".to_string()]
        }
        #[cfg(not(feature = "mock"))]
        {
            vec![]
        }
    };

    let build_args = BuildArgs {
        features,
        ..Default::default()
    };

    // Build the program with the specified arguments
    // Note: SP1_v4's build_programs_with_args does not handle ELF migration
    // Applying a temporary workaround; remove once SP1 supports ELF migration internally
    build_program_with_args(program, build_args);
    migrate_elf(program);

    // Now, ensure cache validity
    let vk = ensure_cache_validity(program)
        .expect("Failed to ensure cache validity after building program");
    (vk.hash_u32(), vk.bytes32())
}

#[cfg(debug_assertions)]
fn generate_elf_contents_and_vk_hash(_program: &str) -> ([u32; 8], String) {
    (
        [0u32; 8],
        "0x0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
    )
}

/// Copies the compiled ELF file of the specified program to its cache directory.
#[cfg(not(debug_assertions))]
fn migrate_elf(program: &str) {
    // Get the build directory from the environment
    let sp1_build_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"));

    // Form the path to the program directory
    let program_path = sp1_build_dir.join(program);

    // Fetch metadata for this program
    let metadata = MetadataCommand::new()
        .manifest_path(program_path.join("Cargo.toml"))
        .exec()
        .expect("Failed to get metadata");

    // Use the root package name as the built ELF name
    let built_elf_name = metadata
        .root_package()
        .expect("Failed to get root package")
        .name
        .clone();

    // Create the cache directory
    let cache_dir = program_path.join("cache");
    fs::create_dir_all(&cache_dir).expect("failed to create cache dir");

    // Destination path: cache/program.elf
    let destination_elf_path = cache_dir.join(format!("{}.elf", program));

    // Source path: program/target/elf-compilation/.../release/{built_elf_name}
    let built_elf_path = program_path
        .join("target")
        .join("elf-compilation")
        .join("riscv32im-succinct-zkvm-elf")
        .join("release")
        .join(&built_elf_name);

    eprintln!("Got the source: {:?}", built_elf_path);
    eprintln!("Got the destination: {:?}", destination_elf_path);

    // Copy the file
    fs::copy(&built_elf_path, &destination_elf_path)
        .expect("Failed to copy the built ELF file to the cache directory");
}
