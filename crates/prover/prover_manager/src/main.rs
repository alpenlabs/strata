use guest_builder::GUEST_RETH_STF_ELF;
use risc0_adapter::RiscZeroHost;
use zkvm::{ProverOptions, ZKVMHost};

// RISC0_DEV_MODE=1 RUST_LOG="[executor]=info" cargo run -p prover_manager

fn main() {
    let pops = ProverOptions {
        // use_mock_prover: false,
        ..Default::default()
    };
    let prover = RiscZeroHost::init(GUEST_RETH_STF_ELF.into(), pops);
    let _ = prover.prove().unwrap();
}
