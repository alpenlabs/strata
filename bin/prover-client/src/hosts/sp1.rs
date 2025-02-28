use std::sync::LazyLock;

use cfg_if::cfg_if;
use strata_primitives::proof::ProofContext;
#[cfg(feature = "sp1-builder")]
use strata_sp1_guest_builder::*;
use zkaleido_sp1_adapter::SP1Host;

cfg_if! {
    if #[cfg(not(feature = "sp1-builder"))] {
        use std::env;
        pub static ELF_BASE_PATH: LazyLock<String> = LazyLock::new(|| {
            env::var("ELF_BASE_PATH").unwrap_or_else(|_| "elfs/sp1".to_string())
        });
    }
}

// BTC_BLOCKSPACE_HOST
cfg_if! {
    if #[cfg(feature = "sp1-builder")] {
        pub static BTC_BLOCKSPACE_HOST: LazyLock<SP1Host> =
            LazyLock::new(|| SP1Host::new_from_pk_bytes(&GUEST_BTC_BLOCKSPACE_PK));
    } else {
        pub static BTC_BLOCKSPACE_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
            let elf_path = format!("{}/guest-btc-blockspace.elf", &*ELF_BASE_PATH);
            let elf = std::fs::read(&elf_path)
                .expect(&format!("Failed to read ELF file from {}", elf_path));
            SP1Host::init(&elf)
        });
    }
}

// EVM_EE_STF_HOST
cfg_if! {
    if #[cfg(feature = "sp1-builder")] {
        pub static EVM_EE_STF_HOST: LazyLock<SP1Host> =
            LazyLock::new(|| SP1Host::new_from_pk_bytes(&GUEST_EVM_EE_STF_PK));
    } else {
        pub static EVM_EE_STF_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
            let elf_path = format!("{}/guest-evm-ee-stf.elf", &*ELF_BASE_PATH);
            let elf = std::fs::read(&elf_path)
                .expect(&format!("Failed to read ELF file from {}", elf_path));
            SP1Host::init(&elf)
        });
    }
}

// CL_STF_HOST
cfg_if! {
    if #[cfg(feature = "sp1-builder")] {
        pub static CL_STF_HOST: LazyLock<SP1Host> =
            LazyLock::new(|| SP1Host::new_from_pk_bytes(&GUEST_CL_STF_PK));
    } else {
        pub static CL_STF_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
            let elf_path = format!("{}/guest-cl-stf.elf", &*ELF_BASE_PATH);
            let elf = std::fs::read(&elf_path)
                .expect(&format!("Failed to read ELF file from {}", elf_path));
            SP1Host::init(&elf)
        });
    }
}

// CHECKPOINT_HOST
cfg_if! {
    if #[cfg(feature = "sp1-builder")] {
        pub static CHECKPOINT_HOST: LazyLock<SP1Host> =
            LazyLock::new(|| SP1Host::new_from_pk_bytes(&GUEST_CHECKPOINT_PK));
    } else {
        pub static CHECKPOINT_HOST: LazyLock<SP1Host> = LazyLock::new(|| {
            let elf_path = format!("{}/guest-checkpoint.elf", &*ELF_BASE_PATH);
            let elf = std::fs::read(&elf_path)
                .expect(&format!("Failed to read ELF file from {}", elf_path));
            SP1Host::init(&elf)
        });
    }
}

/// Returns a reference to the appropriate `SP1Host` instance based on the given [`ProofContext`].
///
/// This function maps the `ProofContext` variant to its corresponding static [`SP1Host`]
/// instance, allowing for efficient host selection for different proof types.
pub fn get_host(id: &ProofContext) -> &'static SP1Host {
    match id {
        ProofContext::BtcBlockspace(..) => &BTC_BLOCKSPACE_HOST,
        ProofContext::EvmEeStf(..) => &EVM_EE_STF_HOST,
        ProofContext::ClStf(..) => &CL_STF_HOST,
        ProofContext::Checkpoint(..) => &CHECKPOINT_HOST,
    }
}
