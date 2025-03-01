use std::str::FromStr;

use clap::ValueEnum;

mod btc_blockscan;
// mod checkpoint;
// mod cl_stf;
mod evm_ee;

use crate::PerformanceReport;

#[derive(Debug, Clone, ValueEnum)]
#[non_exhaustive]
pub enum GuestProgram {
    BtcBlockscan,
    EvmEeStf,
    // ClStf,
    // Checkpoint,
}

impl FromStr for GuestProgram {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "btc-blockscan" => Ok(GuestProgram::BtcBlockscan),
            "evm-ee-stf" => Ok(GuestProgram::EvmEeStf),
            // "cl-stf" => Ok(GuestProgram::ClStf),
            // "checkpoint" => Ok(GuestProgram::Checkpoint),
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
            GuestProgram::BtcBlockscan => btc_blockscan::sp1_btc_blockscan_report(),
            GuestProgram::EvmEeStf => evm_ee::sp1_evm_ee_report(),
            // GuestProgram::ClStf => cl_stf::sp1_cl_stf_report(),
            // GuestProgram::Checkpoint => checkpoint::sp1_checkpoint_report(),
        })
        .map(Into::into)
        .collect()
}

/// Runs Risc0 programs to generate reports.
///
/// Generates [`PerformanceReport`] for each invocation.
#[cfg(feature = "risc0")]
pub fn run_risc0_programs(programs: &[GuestProgram]) -> Vec<PerformanceReport> {
    programs
        .iter()
        .map(|program| match program {
            GuestProgram::BtcBlockscan => btc_blockscan::risc0_btc_blockscan_report(),
            GuestProgram::EvmEeStf => evm_ee::risc0_evm_ee_report(),
            // GuestProgram::ClStf => cl_stf::risc0_cl_stf_report(),
            // GuestProgram::Checkpoint => checkpoint::risc0_checkpoint_report(),
        })
        .map(Into::into)
        .collect()
}
