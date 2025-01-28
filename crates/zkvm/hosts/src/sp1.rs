use std::sync::LazyLock;

use strata_sp1_adapter::SP1Host;
#[cfg(feature = "sp1")]
use strata_sp1_guest_builder::*;

use crate::ProofVm;

pub static BTC_BLOCKSPACE_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    #[cfg(feature = "sp1")]
    {
        SP1Host::new_from_bytes(
            &GUEST_BTC_BLOCKSPACE_ELF,
            &GUEST_BTC_BLOCKSPACE_PK,
            &GUEST_BTC_BLOCKSPACE_VK,
        )
    }

    #[cfg(not(feature = "sp1"))]
    {
        let workspace_root = env!("CARGO_MANIFEST_DIR");
        let file_path = Path::new(workspace_root)
            .join("elf")
            .join("guest-btc-blockspace.elf");
        let elf = fs::read(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", file_path.display()));

        SP1Host::init(&elf)
    }
});

pub static L1_BATCH_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    #[cfg(feature = "sp1")]
    {
        SP1Host::new_from_bytes(&GUEST_L1_BATCH_ELF, &GUEST_L1_BATCH_PK, &GUEST_L1_BATCH_VK)
    }

    #[cfg(not(feature = "sp1"))]
    {
        let workspace_root = env!("CARGO_MANIFEST_DIR");
        let file_path = Path::new(workspace_root)
            .join("elf")
            .join("guest-btc-blockspace.elf");
        let elf = fs::read(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", file_path.display()));

        SP1Host::init(&elf)
    }
});

pub static EVM_EE_STF_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    #[cfg(feature = "sp1")]
    {
        SP1Host::new_from_bytes(
            &GUEST_EVM_EE_STF_ELF,
            &GUEST_EVM_EE_STF_PK,
            &GUEST_EVM_EE_STF_VK,
        )
    }

    #[cfg(not(feature = "sp1"))]
    {
        let workspace_root = env!("CARGO_MANIFEST_DIR");
        let file_path = Path::new(workspace_root)
            .join("elf")
            .join("guest-btc-blockspace.elf");
        let elf = fs::read(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", file_path.display()));

        SP1Host::init(&elf)
    }
});

pub static CL_STF_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    #[cfg(feature = "sp1")]
    {
        SP1Host::new_from_bytes(&GUEST_CL_STF_ELF, &GUEST_CL_STF_PK, &GUEST_CL_STF_VK)
    }

    #[cfg(not(feature = "sp1"))]
    {
        let workspace_root = env!("CARGO_MANIFEST_DIR");
        let file_path = Path::new(workspace_root)
            .join("elf")
            .join("guest-btc-blockspace.elf");
        let elf = fs::read(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", file_path.display()));

        SP1Host::init(&elf)
    }
});

pub static CL_AGG_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    #[cfg(feature = "sp1")]
    {
        SP1Host::new_from_bytes(&GUEST_CL_AGG_ELF, &GUEST_CL_AGG_PK, &GUEST_CL_AGG_VK)
    }

    #[cfg(not(feature = "sp1"))]
    {
        let workspace_root = env!("CARGO_MANIFEST_DIR");
        let file_path = Path::new(workspace_root)
            .join("elf")
            .join("guest-btc-blockspace.elf");
        let elf = fs::read(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", file_path.display()));

        SP1Host::init(&elf)
    }
});

pub static CHECKPOINT_HOST: LazyLock<SP1Host> = std::sync::LazyLock::new(|| {
    #[cfg(feature = "sp1")]
    {
        SP1Host::new_from_bytes(
            &GUEST_CHECKPOINT_ELF,
            &GUEST_CHECKPOINT_PK,
            &GUEST_CHECKPOINT_VK,
        )
    }

    #[cfg(not(feature = "sp1"))]
    {
        let workspace_root = env!("CARGO_MANIFEST_DIR");
        let file_path = Path::new(workspace_root)
            .join("elf")
            .join("guest-btc-blockspace.elf");
        let elf = fs::read(&file_path)
            .unwrap_or_else(|_| panic!("Failed to read file: {}", file_path.display()));

        SP1Host::init(&elf)
    }
});

pub fn get_host(vm: ProofVm) -> &'static SP1Host {
    match vm {
        ProofVm::BtcProving => &BTC_BLOCKSPACE_HOST,
        ProofVm::L1Batch => &L1_BATCH_HOST,
        ProofVm::ELProving => &EVM_EE_STF_HOST,
        ProofVm::CLProving => &CL_STF_HOST,
        ProofVm::CLAggregation => &CL_AGG_HOST,
        ProofVm::Checkpoint => &CHECKPOINT_HOST,
    }
}
