#[cfg(feature = "prover")]
pub const GUEST_RETH_STF_ELF: &[u8] =
    include_bytes!("../guest-reth-stf/elf/riscv32im-succinct-zkvm-elf");

#[cfg(not(feature = "prover"))]
pub const GUEST_RETH_STF_ELF: &[u8] = &[32; 8];
