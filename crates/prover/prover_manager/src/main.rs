use guest_builder::GUEST_RETH_STF_ELF;
use risc0_adapter::RiscZeroHost;
use zkvm::{ProverOptions, ZKVMHost};

fn main() {
    let pops = ProverOptions {
        // use_mock_prover: false,
        ..Default::default()
    };
    let prover = RiscZeroHost::init(GUEST_RETH_STF_ELF.into(), pops);
    let _ = prover.prove().unwrap();
}
