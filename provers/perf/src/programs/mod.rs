use std::str::FromStr;

use clap::ValueEnum;

mod btc_blockscan;
mod checkpoint;
mod checkpoint;
mod cl_stf;

use crate::PerformanceReport;

#[derive(Debug, Clone, ValueEnum)]
#[non_exhaustive]
pub enum GuestProgram {
    BtcBlockscan,
    EvmEeStf,
    ClStf,
    Checkpoint,
}

impl FromStr for GuestProgram {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "btc-blockscan" => Ok(GuestProgram::BtcBlockscan),
            "evm-ee-stf" => Ok(GuestProgram::EvmEeStf),
            "cl-stf" => Ok(GuestProgram::ClStf),
            "checkpoint" => Ok(GuestProgram::Checkpoint),
            // Add more matches
            _ => Err(format!("unknown program: {}", s)),
        }
    }
}

/// Runs SP1 programs to generate reports.
///
/// Generates [`PerformanceReport`] for each invocation.
// #[cfg(feature = "sp1")]
pub fn run_sp1_programs(programs: &[GuestProgram]) -> Vec<PerformanceReport> {
    programs
        .iter()
        .map(|program| match program {
            GuestProgram::Fibonacci => fibonacci::sp1_fib_report(),
            GuestProgram::FibonacciComposition => {
                fibonacci_composition::sp1_fib_composition_report()
            }
            GuestProgram::Sha2Chain => sha2::sp1_sha_report(),
            GuestProgram::SchnorrSigVerify => schnorr::sp1_schnorr_sig_verify_report(),
        })
        .map(Into::into)
        .collect()
}

/// Runs Risc0 programs to generate reports.
///
/// Generates [`PerformanceReport`] for each invocation.
// #[cfg(feature = "risc0")]
pub fn run_risc0_programs(programs: &[GuestProgram]) -> Vec<PerformanceReport> {
    programs
        .iter()
        .map(|program| match program {
            GuestProgram::Fibonacci => fibonacci::risc0_fib_report(),
            GuestProgram::FibonacciComposition => {
                fibonacci_composition::risc0_fib_composition_report()
            }
            GuestProgram::Sha2Chain => sha2::risc0_sha_report(),
            GuestProgram::SchnorrSigVerify => schnorr::risc0_schnorr_sig_verify_report(),
        })
        .map(Into::into)
        .collect()
}
