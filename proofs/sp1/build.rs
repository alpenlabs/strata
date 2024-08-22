use sp1_helper::build_program;
use std::env;

fn main() {
    if cfg!(feature = "prover") {
        const RISC_V_COMPILER: &str = "/opt/riscv/bin/riscv-none-elf-gcc";
        env::set_var("CC_riscv32im_succinct_zkvm_elf", RISC_V_COMPILER);

        build_program("guest-reth-stf")
    }
}
