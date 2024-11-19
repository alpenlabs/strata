use std::fmt;

/// Prover config of the ZkVm Host
#[derive(Debug, Clone, Copy)]
pub struct ProverOptions {
    pub enable_compression: bool,
    pub use_mock_prover: bool,
    pub stark_to_snark_conversion: bool,
    pub use_cached_keys: bool,
}

// Compact representation of the prover options
// Can be used to identify the saved proofs
impl fmt::Display for ProverOptions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut has_flags = false;

        if self.enable_compression {
            write!(f, "c")?;
            has_flags = true;
        }

        if self.use_mock_prover {
            write!(f, "m")?;
            has_flags = true;
        }

        if self.stark_to_snark_conversion {
            write!(f, "s")?;
            has_flags = true;
        }

        if has_flags {
            write!(f, "_")?;
        }

        Ok(())
    }
}

impl Default for ProverOptions {
    fn default() -> Self {
        Self {
            enable_compression: false,
            use_mock_prover: true,
            stark_to_snark_conversion: false,
            use_cached_keys: true,
        }
    }
}
