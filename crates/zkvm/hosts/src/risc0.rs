use std::sync::LazyLock;

use cfg_if::cfg_if;
use strata_primitives::proof::ProofContext;
#[cfg(feature = "risc0-builder")]
use strata_risc0_guest_builder::{
    GUEST_RISC0_BTC_BLOCKSPACE_ELF, GUEST_RISC0_CHECKPOINT_ELF, GUEST_RISC0_CL_STF_ELF,
    GUEST_RISC0_EVM_EE_STF_ELF,
};
use zkaleido_risc0_host::Risc0Host;

pub static ELF_BASE_PATH: LazyLock<String> =
    LazyLock::new(|| std::env::var("ELF_BASE_PATH").unwrap_or_else(|_| "elfs/risc0".to_string()));

// BTC_BLOCKSPACE_HOST
cfg_if! {
    if #[cfg(feature = "risc0-builder")] {
        pub static BTC_BLOCKSPACE_HOST: LazyLock<Risc0Host> =
            LazyLock::new(|| Risc0Host::init(GUEST_RISC0_BTC_BLOCKSPACE_ELF));
    } else {
        pub static BTC_BLOCKSPACE_HOST: LazyLock<Risc0Host> = LazyLock::new(|| {
            let elf_path = format!("{}/guest-btc-blockspace.elf", &*ELF_BASE_PATH);
            let elf = std::fs::read(&elf_path)
                .expect(&format!("Failed to read ELF file from {}", elf_path));
            Risc0Host::init(&elf)
        });
    }
}

// EVM_EE_STF_HOST
cfg_if! {
    if #[cfg(feature = "risc0-builder")] {
        pub static EVM_EE_STF_HOST: LazyLock<Risc0Host> =
            LazyLock::new(|| Risc0Host::init(GUEST_RISC0_EVM_EE_STF_ELF));
    } else {
        pub static EVM_EE_STF_HOST: LazyLock<Risc0Host> = LazyLock::new(|| {
            let elf_path = format!("{}/guest-evm-ee-stf.elf", &*ELF_BASE_PATH);
            let elf = std::fs::read(&elf_path)
                .expect(&format!("Failed to read ELF file from {}", elf_path));
            Risc0Host::init(&elf)
        });
    }
}

// CL_STF_HOST
cfg_if! {
    if #[cfg(feature = "risc0-builder")] {
        pub static CL_STF_HOST: LazyLock<Risc0Host> =
            LazyLock::new(|| Risc0Host::init(GUEST_RISC0_CL_STF_ELF));
    } else {
        pub static CL_STF_HOST: LazyLock<Risc0Host> = LazyLock::new(|| {
            let elf_path = format!("{}/guest-cl-stf.elf", &*ELF_BASE_PATH);
            let elf = std::fs::read(&elf_path)
                .expect(&format!("Failed to read ELF file from {}", elf_path));
            Risc0Host::init(&elf)
        });
    }
}

// CHECKPOINT_HOST
cfg_if! {
    if #[cfg(feature = "risc0-builder")] {
        pub static CHECKPOINT_HOST: LazyLock<Risc0Host> =
            LazyLock::new(|| Risc0Host::init(GUEST_RISC0_CHECKPOINT_ELF));
    } else {
        pub static CHECKPOINT_HOST: LazyLock<Risc0Host> = LazyLock::new(|| {
            let elf_path = format!("{}/guest-checkpoint.elf", &*ELF_BASE_PATH);
            let elf = std::fs::read(&elf_path)
                .expect(&format!("Failed to read ELF file from {}", elf_path));
            Risc0Host::init(&elf)
        });
    }
}
/// Returns a reference to the appropriate [`Risc0Host`] instance based on the given
/// [`ProofContext`].
///
/// This function maps the [`ProofContext`] variant to its corresponding static [`Risc0Host`]
/// instance, allowing for efficient host selection for different proof types.
pub fn get_host(id: &ProofContext) -> &'static Risc0Host {
    match id {
        ProofContext::BtcBlockspace(..) => &BTC_BLOCKSPACE_HOST,
        ProofContext::EvmEeStf(..) => &EVM_EE_STF_HOST,
        ProofContext::ClStf(..) => &CL_STF_HOST,
        ProofContext::Checkpoint(..) => &CHECKPOINT_HOST,
    }
}
