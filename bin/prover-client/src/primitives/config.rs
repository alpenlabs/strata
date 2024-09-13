use std::env;

pub const NUM_PROVER_WORKER: usize = 10;

/// Represents the possible modes of execution for a zkVM program
#[derive(Debug, Clone)]
pub enum ProofGenConfig {
    /// Skips proving.
    Skip,
    /// The executor runs the rollup verification logic in the zkVM, but does not actually
    /// produce a zk proof
    Execute,

    /// The prover runs the rollup verification logic in the zkVM and produces a zk proof
    Prover,
}

impl Default for ProofGenConfig {
    fn default() -> Self {
        match env::var("PROVER_MODE").as_deref() {
            Ok("SKIP") => ProofGenConfig::Skip,
            Ok("Prover") => ProofGenConfig::Prover,
            _ => ProofGenConfig::Execute,
        }
    }
}
