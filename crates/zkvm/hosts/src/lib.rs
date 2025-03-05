use strata_primitives::proof::ProofContext;

#[macro_use]
extern crate cfg_if;

cfg_if! {
    if #[cfg(feature = "native")] {
        pub mod native;
        use zkaleido_native_adapter::NativeHost;

        pub fn get_native_host(ctx: &ProofContext) -> &'static NativeHost {
            native::get_host(ctx)
        }
    }
}

cfg_if! {
    if #[cfg(feature = "risc0")] {
        pub mod risc0;
        use zkaleido_risc0_host::Risc0Host;

        pub fn get_risc0_host(ctx: &ProofContext) -> &'static Risc0Host {
            risc0::get_host(ctx)
        }
    }
}

cfg_if! {
    if #[cfg(feature = "sp1")] {
        pub mod sp1;
        use zkaleido_sp1_host::SP1Host;

        pub fn get_sp1_host(ctx: &ProofContext) -> &'static SP1Host {
            sp1::get_host(ctx)
        }
    }
}
