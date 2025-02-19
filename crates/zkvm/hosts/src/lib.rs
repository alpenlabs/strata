#[macro_use]
extern crate cfg_if;

cfg_if! {
    if #[cfg(feature = "native")] {
        pub mod native;
        use zkaleido_native_adapter::NativeHost;

        pub fn get_native_host(vm: ProofVm) -> &'static NativeHost {
            native::get_host(vm)
        }
    }
}

cfg_if! {
    if #[cfg(feature = "risc0")] {
        pub mod risc0;
        use zkaleido_risc0_adapter::Risc0Host;

        pub fn get_risc0_host(vm: ProofVm) -> &'static Risc0Host {
            risc0::get_host(vm)
        }
    }
}

cfg_if! {
    if #[cfg(feature = "sp1")] {
        pub mod sp1;
        use zkaleido_sp1_adapter::SP1Host;

        pub fn get_sp1_host(vm: ProofVm) -> &'static SP1Host {
            sp1::get_host(vm)
        }
    }
}

/// An identifier of different prover types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofVm {
    BtcProving,
    ELProving,
    CLProving,
    CLAggregation,
    Checkpoint,
}
